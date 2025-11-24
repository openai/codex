<!-- cargo-sync-readme start -->

The purpose of this crate is to make it a bit more ergonomic for portable
applications that need to work with the platform level `RawFd` and
`RawHandle` types.

Rather than conditionally using `RawFd` and `RawHandle`, the `FileDescriptor`
type can be used to manage ownership, duplicate, read and write.

## FileDescriptor

This is a bit of a contrived example, but demonstrates how to avoid
the conditional code that would otherwise be required to deal with
calling `as_raw_fd` and `as_raw_handle`:

```
use filedescriptor::{FileDescriptor, FromRawFileDescriptor, Result};
use std::io::Write;

fn get_stdout() -> Result<FileDescriptor> {
  let stdout = std::io::stdout();
  let handle = stdout.lock();
  FileDescriptor::dup(&handle)
}

fn print_something() -> Result<()> {
   get_stdout()?.write(b"hello")?;
   Ok(())
}
```

## Pipe
The `Pipe` type makes it more convenient to create a pipe and manage
the lifetime of both the read and write ends of that pipe.

```
use filedescriptor::Pipe;
use std::io::{Read, Write};

let mut pipe = Pipe::new()?;
pipe.write.write(b"hello")?;
drop(pipe.write);

let mut s = String::new();
pipe.read.read_to_string(&mut s)?;
assert_eq!(s, "hello");
```

## Socketpair
The `socketpair` function returns a pair of connected `SOCK_STREAM`
sockets and functions both on posix and windows systems.

```
use std::io::{Read, Write};

let (mut a, mut b) = filedescriptor::socketpair()?;
a.write(b"hello")?;
drop(a);

let mut s = String::new();
b.read_to_string(&mut s)?;
assert_eq!(s, "hello");
```

## Polling
The `mio` crate offers powerful and scalable IO multiplexing, but there
are some situations where `mio` doesn't fit.  The `filedescriptor` crate
offers a `poll(2)` compatible interface suitable for testing the readiness
of a set of file descriptors.  On unix systems this is a very thin wrapper
around `poll(2)`, except on macOS where it is actually a wrapper around
the `select(2)` interface.  On Windows systems the winsock `WSAPoll`
function is used instead.

```
use filedescriptor::*;
use std::time::Duration;
use std::io::{Read, Write};

let (mut a, mut b) = filedescriptor::socketpair()?;
let mut poll_array = [pollfd {
   fd: a.as_socket_descriptor(),
   events: POLLIN,
   revents: 0
}];
// sleeps for 20 milliseconds because `a` is not yet ready
assert_eq!(poll(&mut poll_array, Some(Duration::from_millis(20)))?, 0);

b.write(b"hello")?;

// Now a is ready for read
assert_eq!(poll(&mut poll_array, Some(Duration::from_millis(20)))?, 1);

```

<!-- cargo-sync-readme end -->
