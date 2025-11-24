<!-- Serde readme rendered on crates.io -->

**Serde is a framework for *ser*ializing and *de*serializing Rust data structures efficiently and generically.**

---

You may be looking for:

- [An overview of Serde](https://serde.rs)
- [Data formats supported by Serde](https://serde.rs/#data-formats)
- [Setting up `#[derive(Serialize, Deserialize)]`](https://serde.rs/derive.html)
- [Examples](https://serde.rs/examples.html)
- [API documentation](https://docs.rs/serde)
- [Release notes](https://github.com/serde-rs/serde/releases)

## Serde in action

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let point = Point { x: 1, y: 2 };

    // Convert the Point to a JSON string.
    let serialized = serde_json::to_string(&point).unwrap();

    // Prints serialized = {"x":1,"y":2}
    println!("serialized = {}", serialized);

    // Convert the JSON string back to a Point.
    let deserialized: Point = serde_json::from_str(&serialized).unwrap();

    // Prints deserialized = Point { x: 1, y: 2 }
    println!("deserialized = {:?}", deserialized);
}
```

## Getting help

Serde is one of the most widely used Rust libraries so any place that Rustaceans
congregate will be able to help you out. For chat, consider trying the
[#rust-questions] or [#rust-beginners] channels of the unofficial community
Discord (invite: <https://discord.gg/rust-lang-community>), the [#rust-usage]
or [#beginners] channels of the official Rust Project Discord (invite:
<https://discord.gg/rust-lang>), or the [#general][zulip] stream in Zulip. For
asynchronous, consider the [\[rust\] tag on StackOverflow][stackoverflow], the
[/r/rust] subreddit which has a pinned weekly easy questions post, or the Rust
[Discourse forum][discourse]. It's acceptable to file a support issue in this
repo but they tend not to get as many eyes as any of the above and may get
closed without a response after some time.

[#rust-questions]: https://discord.com/channels/273534239310479360/274215136414400513
[#rust-beginners]: https://discord.com/channels/273534239310479360/273541522815713281
[#rust-usage]: https://discord.com/channels/442252698964721669/443150878111694848
[#beginners]: https://discord.com/channels/442252698964721669/448238009733742612
[zulip]: https://rust-lang.zulipchat.com/#narrow/stream/122651-general
[stackoverflow]: https://stackoverflow.com/questions/tagged/rust
[/r/rust]: https://www.reddit.com/r/rust
[discourse]: https://users.rust-lang.org
