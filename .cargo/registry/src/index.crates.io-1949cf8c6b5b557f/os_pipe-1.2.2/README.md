# os_pipe.rs [![Actions Status](https://github.com/oconnor663/os_pipe.rs/workflows/tests/badge.svg)](https://github.com/oconnor663/os_pipe.rs/actions) [![crates.io](https://img.shields.io/crates/v/os_pipe.svg)](https://crates.io/crates/os_pipe) [![docs.rs](https://docs.rs/os_pipe/badge.svg)](https://docs.rs/os_pipe)

A cross-platform library for opening OS pipes, like those from
[`pipe`](https://man7.org/linux/man-pages/man2/pipe.2.html) on Linux or
[`CreatePipe`](https://docs.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createpipe)
on Windows. The Rust standard library provides
[`Stdio::piped`](https://doc.rust-lang.org/std/process/struct.Stdio.html#method.piped) for
simple use cases involving child processes, ~~but it doesn't support creating pipes directly.
This crate fills that gap.~~ **Update:** Rust 1.87 added
[`std::io::pipe`](https://doc.rust-lang.org/std/io/fn.pipe.html), so this crate is no longer
needed except to support older compiler versions.

- [Docs](https://docs.rs/os_pipe)
- [Crate](https://crates.io/crates/os_pipe)
- [Repo](https://github.com/oconnor663/os_pipe.rs)

## Common deadlocks related to pipes

When you work with pipes, you often end up debugging a deadlock at
some point. These can be confusing if you don't know why they
happen. Here are two things you need to know:

1. Pipe reads block until some bytes are written or all writers are
   closed. **If you forget to close a writer, reads can block
   forever.** This includes writers inside a
   [`std::process::Command`](https://doc.rust-lang.org/std/process/struct.Command.html)
   object or writers given to child processes.
2. Pipes have an internal buffer of some fixed size. On Linux for
   example, pipe buffers are 64 KiB by default. Pipe writes block
   until buffer space is available or all readers are closed. **If
   you have readers open but not reading, writes can block
   forever.**

Deadlocked reads caused by a forgotten writer usually show up
immediately, which makes them relatively easy to fix once you know
what to look for. (See "Avoid a deadlock!" in the example code
below.) However, deadlocked writes caused by full pipe buffers are
trickier. These might only show up for larger inputs, and they might
be timing-dependent or platform-dependent. If you find that writing
to a pipe deadlocks sometimes, think about who's supposed to be
reading from that pipe and whether that thread or process might be
blocked on something else. For more on this, see the [Gotchas
Doc](https://github.com/oconnor663/duct.py/blob/master/gotchas.md#using-io-threads-to-avoid-blocking-children)
from the [`duct`](https://github.com/oconnor663/duct.rs) crate. (And
consider whether [`duct`](https://github.com/oconnor663/duct.rs)
might be a good fit for your use case.)

## Examples

Here we write a single byte into a pipe and read it back out:

```rust
use std::io::prelude::*;

let (mut reader, mut writer) = os_pipe::pipe()?;
// XXX: If this write blocks, we'll never get to the read.
writer.write_all(b"x")?;
let mut output = [0];
reader.read_exact(&mut output)?;
assert_eq!(b"x", &output);
```

This is a minimal working example, but as discussed in the section
above, reading and writing on the same thread like this is
deadlock-prone. If we wrote 100 KB instead of just one byte, this
example would block on `write_all`, it would never make it to
`read_exact`, and that would be a deadlock. Doing the read and write
from different threads or different processes would fix the
deadlock.

For a more complex example, here we join the stdout and stderr of a
child process into a single pipe. To do that we open a pipe, clone
its writer, and set that pair of writers as the child's stdout and
stderr. (This is possible because `PipeWriter` implements
`Into<Stdio>`.) Then we can read interleaved output from the pipe
reader. This example is deadlock-free, but note the comment about
closing the writers.

```rust
// We're going to spawn a child process that prints "foo" to stdout
// and "bar" to stderr, and we'll combine these into a single pipe.
let mut command = std::process::Command::new("python");
command.args(&["-c", r#"
import sys
sys.stdout.write("foo")
sys.stdout.flush()
sys.stderr.write("bar")
sys.stderr.flush()
"#]);

// Here's the interesting part. Open a pipe, clone its writer, and
// set that pair of writers as the child's stdout and stderr.
let (mut reader, writer) = os_pipe::pipe()?;
let writer_clone = writer.try_clone()?;
command.stdout(writer);
command.stderr(writer_clone);

// Now start the child process running.
let mut handle = command.spawn()?;

// Avoid a deadlock! This parent process is still holding open pipe
// writers inside the Command object, and we have to close those
// before we read. Here we do this by dropping the Command object.
drop(command);

// Finally we can read all the output and clean up the child.
let mut output = String::new();
reader.read_to_string(&mut output)?;
handle.wait()?;
assert_eq!(output, "foobar");
```

Note that the [`duct`](https://github.com/oconnor663/duct.rs) crate
can reproduce the example above in a single line of code, with no
risk of deadlocks and no risk of leaking [zombie
children](https://en.wikipedia.org/wiki/Zombie_process).
