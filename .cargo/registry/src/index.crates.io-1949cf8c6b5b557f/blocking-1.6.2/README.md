# blocking

[![Build](https://github.com/smol-rs/blocking/workflows/Build%20and%20test/badge.svg)](
https://github.com/smol-rs/blocking/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/blocking)
[![Cargo](https://img.shields.io/crates/v/blocking.svg)](
https://crates.io/crates/blocking)
[![Documentation](https://docs.rs/blocking/badge.svg)](
https://docs.rs/blocking)

A thread pool for isolating blocking I/O in async programs.

Sometimes there's no way to avoid blocking I/O. Consider files or stdin, which have weak async
support on modern operating systems. While [IOCP], [AIO], and [io_uring] are possible
solutions, they're not always available or ideal.

Since blocking is not allowed inside futures, we must move blocking I/O onto a special thread
pool provided by this crate. The pool dynamically spawns and stops threads depending on the
current number of running I/O jobs.

Note that there is a limit on the number of active threads. Once that limit is hit, a running
job has to finish before others get a chance to run. When a thread is idle, it waits for the
next job or shuts down after a certain timeout.

The default number of threads (set to 500) can be altered by setting BLOCKING_MAX_THREADS environment variable with value between 1 and 10000.

[IOCP]: https://en.wikipedia.org/wiki/Input/output_completion_port
[AIO]: http://man7.org/linux/man-pages/man2/io_submit.2.html
[io_uring]: https://lwn.net/Articles/776703

## Examples

Read the contents of a file:

```rust
use blocking::unblock;
use std::fs;

let contents = unblock(|| fs::read_to_string("file.txt")).await?;
println!("{}", contents);
```

Read a file and pipe its contents to stdout:

```rust
use blocking::{unblock, Unblock};
use futures_lite::io;
use std::fs::File;

let input = unblock(|| File::open("file.txt")).await?;
let input = Unblock::new(input);
let mut output = Unblock::new(std::io::stdout());

io::copy(input, &mut output).await?;
```

Iterate over the contents of a directory:

```rust
use blocking::Unblock;
use futures_lite::prelude::*;
use std::fs;

let mut dir = Unblock::new(fs::read_dir(".")?);
while let Some(item) = dir.next().await {
    println!("{}", item?.file_name().to_string_lossy());
}
```

Spawn a process:

```rust
use blocking::unblock;
use std::process::Command;

let out = unblock(|| Command::new("dir").output()).await?;
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
