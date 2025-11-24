# zbus

[![](https://docs.rs/zbus/badge.svg)](https://docs.rs/zbus/) [![](https://img.shields.io/crates/v/zbus)](https://crates.io/crates/zbus)

This is the main subcrate of the [zbus] project, that provides the API to interact with D-Bus. It
takes care of the establishment of a connection, the creation, sending and receiving of different
kind of D-Bus messages (method calls, signals etc) for you.

**Status:** Stable.

## Getting Started

The best way to get started with zbus is the [book](https://dbus2.github.io/zbus/), where we start
with basic D-Bus concepts and explain with code samples, how zbus makes D-Bus easy.

## Example code

We'll create a simple D-Bus service and client to demonstrate the usage of zbus. Note that these
examples assume that a D-Bus broker is setup on your machine and you've a session bus running
(`DBUS_SESSION_BUS_ADDRESS` environment variable must be set). This is guaranteed to be the case on
a typical Linux desktop session.

### Service

A simple service that politely greets whoever calls its `SayHello` method:

```rust,no_run
use std::{error::Error, future::pending};
use zbus::{connection, interface};

struct Greeter {
    count: u64
}

#[interface(name = "org.zbus.MyGreeter1")]
impl Greeter {
    // Can be `async` as well.
    fn say_hello(&mut self, name: &str) -> String {
        self.count += 1;
        format!("Hello {}! I have been called {} times.", name, self.count)
    }
}

// Although we use `tokio` here, you can use any async runtime of choice.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let greeter = Greeter { count: 0 };
    let _conn = connection::Builder::session()?
        .name("org.zbus.MyGreeter")?
        .serve_at("/org/zbus/MyGreeter", greeter)?
        .build()
        .await?;

    // Do other things or go to wait forever
    pending::<()>().await;

    Ok(())
}
```

You can use the following command to test it:

```bash
$ busctl --user call org.zbus.MyGreeter /org/zbus/MyGreeter org.zbus.MyGreeter1 SayHello s "Maria"
s "Hello Maria! I have been called 1 times."
```

### Client

Now let's write the client-side code for `MyGreeter` service:

```rust,no_run
use zbus::{Connection, Result, proxy};

#[proxy(
    interface = "org.zbus.MyGreeter1",
    default_service = "org.zbus.MyGreeter",
    default_path = "/org/zbus/MyGreeter"
)]
trait MyGreeter {
    async fn say_hello(&self, name: &str) -> Result<String>;
}

// Although we use `tokio` here, you can use any async runtime of choice.
#[tokio::main]
async fn main() -> Result<()> {
    let connection = Connection::session().await?;

    // `proxy` macro creates `MyGreaterProxy` based on `Notifications` trait.
    let proxy = MyGreeterProxy::new(&connection).await?;
    let reply = proxy.say_hello("Maria").await?;
    println!("{reply}");

    Ok(())
}
```

## Blocking API

While zbus is primarily asynchronous (since 2.0), [blocking wrappers][bw] are provided for
convenience.

## Compatibility with async runtimes

zbus is runtime-agnostic and should work out of the box with different Rust async runtimes. However,
in order to achieve that, zbus spawns a thread per connection to handle various internal tasks. If
that is something you would like to avoid, you need to:

* Use [`connection::Builder`] and disable the `internal_executor` flag.
* Ensure the [internal executor keeps ticking continuously][iektc].

Moreover, by default zbus makes use of [`async-io`] for all I/O, which also launches its own thread
to run its own internal executor.

### Special tokio support

Since [`tokio`] is the most popular async runtime, zbus provides an easy way to enable tight
integration with it without you having to worry about any of the above: Enabling the `tokio` feature:

```toml
# Sample Cargo.toml snippet.
[dependencies]
# Also disable the default `async-io` feature to avoid unused dependencies.
zbus = { version = "3", default-features = false, features = ["tokio"] }
```

That's it! No threads launched behind your back by zbus (directly or indirectly) now and no need to
tick any executors etc. 😼

**Note**: On Windows, the `async-io` feature is currently required for UNIX domain socket support,
see [the corresponding tokio issue on GitHub][tctiog].

[zbus]: https://github.com/dbus2/zbus\#readme
[bw]: https://docs.rs/zbus/latest/zbus/blocking/index.html
[iektc]: https://docs.rs/zbus/latest/zbus/connection/struct.Connection.html#examples-1
[tctiog]: https://github.com/tokio-rs/tokio/issues/2201
[`connection::Builder`]: https://docs.rs/zbus/latest/zbus/connection/struct.ConnectionBuilder.html
[`tokio`]: https://crates.io/crates/tokio
[`async-io`]: https://crates.io/crates/async-io
