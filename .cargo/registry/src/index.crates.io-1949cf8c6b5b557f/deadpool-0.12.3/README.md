# Deadpool [![Latest Version](https://img.shields.io/crates/v/deadpool.svg)](https://crates.io/crates/deadpool) [![Build Status](https://img.shields.io/github/actions/workflow/status/bikeshedder/deadpool/ci.yml?branch=main)](https://github.com/bikeshedder/deadpool/actions?query=workflow%3ARust) ![Unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg "Unsafe forbidden") [![Rust 1.75+](https://img.shields.io/badge/rustc-1.75+-lightgray.svg "Rust 1.75+")](https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html)


Deadpool is a dead simple async pool for connections and objects
of any type.

This crate provides two implementations:

- Managed pool (`deadpool::managed::Pool`)
  - Creates and recycles objects as needed
  - Useful for [database connection pools](#database-connection-pools)
  - Enabled via the `managed` feature in your `Cargo.toml`

- Unmanaged pool (`deadpool::unmanaged::Pool`)
  - All objects either need to be created by the user and added to the
    pool manually. It is also possible to create a pool from an existing
    collection of objects.
  - Enabled via the `unmanaged` feature in your `Cargo.toml`

## Features

| Feature | Description | Extra dependencies | Default |
| ------- | ----------- | ------------------ | ------- |
| `managed` | Enable managed pool implementation | - | yes |
| `unmanaged` | Enable unmanaged pool implementation | - | yes |
| `rt_tokio_1` | Enable support for [tokio](https://crates.io/crates/tokio) crate | `tokio/time` | no |
| `rt_async-std_1` | Enable support for [async-std](https://crates.io/crates/async-std) crate | `async-std` | no |
| `serde` | Enable support for deserializing pool config | `serde/derive` | no |

The runtime features (`rt_*`) are only needed if you need support for
timeouts. If you try to use timeouts without specifying a runtime at
pool creation the pool get methods will return an
`PoolError::NoRuntimeSpecified` error.

## Managed pool (aka. connection pool)

This is the obvious choice for connection pools of any kind. Deadpool already
comes with a couple of [database connection pools](#database-connection-pools)
which work out of the box.

### Example

```rust
use deadpool::managed;

#[derive(Debug)]
enum Error { Fail }

struct Computer {}

impl Computer {
    async fn get_answer(&self) -> i32 {
        42
    }
}

struct Manager {}

impl managed::Manager for Manager {
    type Type = Computer;
    type Error = Error;

    async fn create(&self) -> Result<Computer, Error> {
        Ok(Computer {})
    }

    async fn recycle(&self, _: &mut Computer, _: &managed::Metrics) -> managed::RecycleResult<Error> {
        Ok(())
    }
}

type Pool = managed::Pool<Manager>;

#[tokio::main]
async fn main() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).build().unwrap();
    let mut conn = pool.get().await.unwrap();
    let answer = conn.get_answer().await;
    assert_eq!(answer, 42);
}
```

### Database connection pools

Deadpool supports various database backends by implementing the
`deadpool::managed::Manager` trait. The following backends are
currently supported:

Database (Protocol) | Backend | Crate | Latest Version | Official [deadpool-rs](https://github.com/deadpool-rs/) crate¹ |
------------------- | ------- | ----- | -------------- | ------------------------------------------------------------- |
[PostgreSQL](https://www.postgresql.org/) | [tokio-postgres](https://crates.io/crates/tokio-postgres) | [deadpool-postgres](https://crates.io/crates/deadpool-postgres) | [![Latest Version](https://img.shields.io/crates/v/deadpool-postgres.svg)](https://crates.io/crates/deadpool-postgres) | ✔ |
[Valkey](https://valkey.io/), [Redis](https://redis.io/) | [redis](https://crates.io/crates/redis) | [deadpool-redis](https://crates.io/crates/deadpool-redis) | [![Latest Version](https://img.shields.io/crates/v/deadpool-redis.svg)](https://crates.io/crates/deadpool-redis) | ✔ |
[SQLite](https://sqlite.org) | [rusqlite](https://crates.io/crates/rusqlite) | [deadpool-sqlite](https://crates.io/crates/deadpool-sqlite) | [![Latest Version](https://img.shields.io/crates/v/deadpool-sqlite.svg)](https://crates.io/crates/deadpool-sqlite) | ✔ |
[RabbitMQ](https://www.rabbitmq.com/) ([AMQP](https://www.amqp.org/)) | [lapin](https://crates.io/crates/lapin) | [deadpool-lapin](https://crates.io/crates/deadpool-lapin) | [![Latest Version](https://img.shields.io/crates/v/deadpool-lapin.svg)](https://crates.io/crates/deadpool-lapin) | ✔ |
[Memcached](https://www.memcached.org/) | [async-memcached](https://crates.io/crates/async-memcached) | [deadpool-memcached](https://crates.io/crates/deadpool-memcached) | [![Latest Version](https://img.shields.io/crates/v/deadpool-memcached.svg)](https://crates.io/crates/deadpool-memcached) | ✔ |
— | [diesel](https://crates.io/crates/diesel) | [deadpool-diesel](https://crates.io/crates/deadpool-diesel) | [![Latest Version](https://img.shields.io/crates/v/deadpool-diesel.svg)](https://crates.io/crates/deadpool-diesel) | ✔ |
— | [r2d2](https://crates.io/crates/r2d2) | [deadpool-r2d2](https://crates.io/crates/deadpool-r2d2) | [![Latest Version](https://img.shields.io/crates/v/deadpool-r2d2.svg)](https://crates.io/crates/deadpool-r2d2) | ✔ |
[libSQL](https://docs.turso.tech/libsql) | [libsql](https://crates.io/crates/libsql) | [deadpool-libsql](https://crates.io/crates/deadpool-libsql) | [![Latest Version](https://img.shields.io/crates/v/deadpool-libsql.svg)](https://crates.io/crates/deadpool-libsql) | ✔ |
[Microsoft SQL Server](https://www.microsoft.com/sql-server/) | [tiberius](https://crates.io/crates/tiberius) | [deadpool-tiberius](https://crates.io/crates/deadpool-tiberius) | [![Latest Version](https://img.shields.io/crates/v/deadpool-tiberius.svg)](https://crates.io/crates/deadpool-tiberius) | |
[neo4j](https://neo4j.com/) ([Bolt](https://neo4j.com/docs/bolt/)) | [bolt-client](https://crates.io/crates/bolt-client) | [deadpool-bolt](https://crates.io/crates/deadpool-bolt) | [![Latest Version](https://img.shields.io/crates/v/deadpool-bolt.svg)](https://crates.io/crates/deadpool-bolt) |  |
[rbatis](https://rbatis.github.io/rbatis.io/) | [rbatis](https://crates.io/crates/rbatis) | [rbatis](https://crates.io/crates/rbatis) | [![Latest Version](https://img.shields.io/crates/v/rbatis.svg)](https://crates.io/crates/rbatis) | |
[LDAP v3](https://www.rfc-editor.org/rfc/rfc4511.txt) | [ldap3](https://crates.io/crates/ldap3) | [deadpool-ldap3](https://crates.io/crates/deadpool-ldap3) | [![Latest Version](https://img.shields.io/crates/v/deadpool-ldap3.svg)](https://crates.io/crates/deadpool-ldap3) | |
[ClickHouse](https://clickhouse.com/) | [clickhouse](https://crates.io/crates/clickhouse) | [clickhouse-connection-pool](https://crates.io/crates/clickhouse-connection-pool) | [![Latest Version](https://img.shields.io/crates/v/clickhouse-connection-pool.svg)](https://crates.io/crates/clickhouse-connection-pool) | |

¹ "Official deadpool-rs crates" marks crates maintained by the [deadpool-rs](https://github.com/deadpool-rs/) project. This shows ownership only, not quality or support level. Third-party crates are welcome to join the deadpool-rs umbrella for centralized maintenance and collaboration.

### Reasons for yet another connection pool

Deadpool is by no means the only pool implementation available. It does
things a little different and that is the main reason for it to exist:

- **Deadpool is compatible with any executor.** Objects are returned to the
  pool using the `Drop` trait. The health of those objects is checked upon
  next retrieval and not when they are returned. Deadpool never performs any
  actions in the background. This is the reason why deadpool does not need
  to spawn futures and does not rely on a background thread or task of any
  type.

- **Identical startup and runtime behaviour**. When writing long running
  application there usually should be no difference between startup and
  runtime if a database connection is temporarily not available. Nobody
  would expect an application to crash if the database becomes unavailable
  at runtime. So it should not crash on startup either. Creating the pool
  never fails and errors are only ever returned when calling `Pool::get()`.

  If you really want your application to crash on startup if objects can
  not be created on startup simply call
  `pool.get().await.expect("DB connection failed")` right after creating
  the pool.

- **Deadpool is fast.** Whenever working with locking primitives they are
  held for the shortest duration possible. When returning an object to the
  pool a single mutex is locked and when retrieving objects from the pool
  a Semaphore is used to make this Mutex as little contested as possible.

- **Deadpool is simple.** Dead simple. There is very little API surface.
  The actual code is barely 100 lines of code and lives in the two functions
  `Pool::get` and `Object::drop`.

- **Deadpool is extensible.** By using `post_create`, `pre_recycle` and
  `post_recycle` hooks you can customize object creation and recycling
  to fit your needs.

- **Deadpool provides insights.** All objects track `Metrics` and the pool
  provides a `status` method that can be used to find out details about
  the inner workings.

- **Deadpool is resizable.** You can grow and shrink the pool at runtime
  without requiring an application restart.


## Unmanaged pool

An unmanaged pool is useful when you can't write a manager for the objects
you want to pool or simply don't want to. This pool implementation is slightly
faster than the managed pool because it does not use a `Manager` trait to
`create` and `recycle` objects but leaves it up to the user.

### Unmanaged pool example

```rust
use deadpool::unmanaged::Pool;

struct Computer {}

impl Computer {
    async fn get_answer(&self) -> i32 {
        42
    }
}

#[tokio::main]
async fn main() {
    let pool = Pool::from(vec![
        Computer {},
        Computer {},
    ]);
    let s = pool.get().await.unwrap();
    assert_eq!(s.get_answer().await, 42);
}
```

## FAQ

### Why does deadpool depend on `tokio`? I thought it was runtime agnostic...

Deadpool depends on `tokio::sync::Semaphore`. This does **not** mean that
the tokio runtime or anything else of tokio is being used or will be part
of your build. You can easily check this by running the following command
in your own code base:

```shell
cargo tree --format "{p} {f}"
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0)>
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT)>

at your option.
