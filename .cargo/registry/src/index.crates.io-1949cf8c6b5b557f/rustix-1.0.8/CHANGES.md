# Changes from 0.38.x to 1.x

## Silent behavior changes

[`rustix::pipe::fcntl_setpipe_size`] now returns the new size, which may be
greater than the requested size.

[`rustix::pipe::fcntl_setpipe_size`]: https://docs.rs/rustix/1/rustix/pipe/fn.fcntl_setpipe_size.html

When a `&mut Vec<_>` is passed to [`rustix::event::epoll::wait`],
[`rustix::event::kqueue::kevent`], or [`rustix::event::port::getn`], these
functions previously adjusted the length of the `Vec` to the number of elements
written, and now do not. A common alternative is to wrap the `&mut Vec<_>`
using [`spare_capacity`], and then to clear the `Vec` by iterating using
`.drain(..)` after each call. For an example of using `spare_capacity` in this
way, see [here].

[`rustix::event::epoll::wait`]: https://docs.rs/rustix/1/rustix/event/epoll/fn.wait.html
[`rustix::event::kqueue::kevent`]: https://docs.rs/rustix/1/x86_64-unknown-freebsd/rustix/event/kqueue/fn.kevent.html
[`rustix::event::port::getn`]: https://docs.rs/rustix/1/x86_64-unknown-illumos/rustix/event/port/fn.getn.html
[`spare_capacity`]: https://docs.rs/rustix/1/rustix/buffer/fn.spare_capacity.html
[here]: https://docs.rs/rustix/1/rustix/event/epoll/index.html#examples

## API changes

`rustix::thread::FutexOperation` and `rustix::thread::futex` are removed. Use
the functions in the [`rustix::thread::futex`] module instead.

[`rustix::thread::futex`]: https://docs.rs/rustix/1/rustix/thread/futex/index.html

[`rustix::process::waitpid`]'s return type changed from `WaitStatus` to
`(Pid, WaitStatus)`, to additionally return the pid of the child.

[`rustix::process::waitpid`]: https://docs.rs/rustix/1/rustix/process/fn.waitpid.html

[`terminating_signal`] and other functions in [`rustix::process::WaitStatus`] changed
from returning `u32` to returning `i32`, for better compatibility with the new
[`Signal`] type and [`exit`].

[`terminating_signal`]: https://docs.rs/rustix/1/rustix/process/struct.WaitStatus.html#method.terminating_signal
[`rustix::process::WaitStatus`]: https://docs.rs/rustix/1/rustix/process/struct.WaitStatus.html
[`Signal`]: https://docs.rs/rustix/1/rustix/process/struct.Signal.html
[`exit`]: std::process::exit

The `SLAVE` flag in [`rustix::mount::MountPropagationFlags`] is renamed to
[`DOWNSTREAM`].

[`rustix::mount::MountPropagationFlags`]: https://docs.rs/rustix/1/rustix/mount/struct.MountPropagationFlags.html
[`DOWNSTREAM`]: https://docs.rs/rustix/1/rustix/mount/struct.MountPropagationFlags.html#associatedconstant.DOWNSTREAM

The "cc" and "libc-extra-traits" features are removed. The "cc" feature hasn't
had any effect for several major releases. If you need the traits provided by
"libc-extra-traits", you should instead depend on libc directly and enable its
"extra_traits" feature.

`rustix::net::Shutdown::ReadWrite` is renamed to
[`rustix::net::Shutdown::Both`] to [align with std].

[`rustix::net::Shutdown::Both`]: https://docs.rs/rustix/1/rustix/net/enum.Shutdown.html#variant.Both
[align with std]: https://doc.rust-lang.org/stable/std/net/enum.Shutdown.html#variant.Both

The `rustix::io_uring::io_uring_register_files_skip` function is replaced with
a [`IORING_REGISTER_FILES_SKIP`] constant, similar to the [`rustix::fs::CWD`]
constant.

[`IORING_REGISTER_FILES_SKIP`]: https://docs.rs/rustix/1/rustix/io_uring/constant.IORING_REGISTER_FILES_SKIP.html
[`rustix::fs::CWD`]: https://docs.rs/rustix/1/rustix/fs/constant.CWD.html

[`rustix::io_uring::io_uring_register`] now has a [`IoringRegisterFlags`]
argument, and `rustix::io_uring::io_uring_register_with` is removed.

[`rustix::io_uring::io_uring_register`]: https://docs.rs/rustix/1/rustix/io_uring/fn.io_uring_register.html
[`IoringRegisterFlags`]: https://docs.rs/rustix/1/rustix/io_uring/struct.IoringRegisterFlags.html

Several structs in [`rustix::io_uring`] are now marked `#[non_exhaustive]`
because they contain padding or reserved fields. Instead of constructing
them with field values and `..Default::default()`, construct them with
`Default::default()` and separately assign the fields.

[`rustix::io_uring`]: https://docs.rs/rustix/1/rustix/io_uring/index.html

[`rustix::process::Resource`], [`rustix::thread::MembarrierCommand`], and
[`rustix::thread::Capability`] are now marked `#[non_exhaustive]` to ease
migration in case new constants are defined in the future.

[`rustix::process::Resource`]: https://docs.rs/rustix/1/rustix/process/enum.Resource.html
[`rustix::thread::MembarrierCommand`]: https://docs.rs/rustix/1/rustix/thread/enum.MembarrierCommand.html
[`rustix::thread::Capability`]: https://docs.rs/rustix/1/rustix/thread/enum.Capability.html

`rustix::process::WaitidOptions` and `rustix::process::WaitidStatus` are
renamed to
[`rustix::process::WaitIdOptions`] and [`rustix::process::WaitIdStatus`] (note
the capitalization), for consistency with [`rustix::process::WaitId`].

[`rustix::process::WaitIdOptions`]: https://docs.rs/rustix/1/rustix/process/struct.WaitIdOptions.html
[`rustix::process::WaitIdStatus`]: https://docs.rs/rustix/1/rustix/process/struct.WaitIdStatus.html
[`rustix::process::WaitId`]: https://docs.rs/rustix/1/rustix/process/enum.WaitId.html

The offsets in [`rustix::fs::SeekFrom::Hole`] and
[`rustix::fs::SeekFrom::Data`] are changed from `i64` to `u64`, to
[align with std], since they represent absolute offsets.

[`rustix::fs::SeekFrom::Hole`]: https://docs.rs/rustix/1/rustix/fs/enum.SeekFrom.html#variant.Hole
[`rustix::fs::SeekFrom::Data`]: https://docs.rs/rustix/1/rustix/fs/enum.SeekFrom.html#variant.Data
[align with std]: https://doc.rust-lang.org/stable/std/io/enum.SeekFrom.html#variant.Start

Functions in [`rustix::net::sockopt`] are renamed to remove the `get_` prefix,
to [align with Rust conventions].

[`rustix::net::sockopt`]: https://docs.rs/rustix/1/rustix/net/sockopt/index.html
[align with Rust conventions]: https://rust-lang.github.io/api-guidelines/naming.html#getter-names-follow-rust-convention-c-getter

`rustix::process::sched_*` and `rustix::process::membarrier_*` are moved from
[`rustix::process`] to [`rustix::thread`], as they operate on the current
thread rather than the current process.

[`rustix::process`]: https://docs.rs/rustix/1/rustix/process/index.html
[`rustix::thread`]: https://docs.rs/rustix/1/rustix/thread/index.html

The `udata` in [`rustix::event::kqueue::Event`] is changed from `isize` to
`*mut c_void` to better propagate pointer provenance. To use arbitrary integer
values, convert using the [`without_provenance_mut`] and the [`.addr()`]
functions.

[`rustix::event::kqueue::Event`]: https://docs.rs/rustix/1/x86_64-unknown-freebsd/rustix/event/kqueue/struct.Event.html
[`without_provenance_mut`]: https://doc.rust-lang.org/stable/std/ptr/fn.without_provenance_mut.html
[`.addr()`]: https://doc.rust-lang.org/stable/std/primitive.pointer.html#method.addr

`rustix::mount::mount_recursive_bind` is renamed to
[`rustix::mount::mount_bind_recursive`]. See [this comment] for details.

[`rustix::mount::mount_bind_recursive`]: https://docs.rs/rustix/1/rustix/mount/fn.mount_bind_recursive.html
[this comment]: https://github.com/bytecodealliance/rustix/pull/763#issuecomment-1662756184

The `rustix::procfs` is removed. This functionality is now available in the
[rustix-linux-procfs crate].

[rustix-linux-procfs crate]: https://crates.io/crates/rustix-linux-procfs

`rustix::net::RecvMsgReturn` is renamed to [`rustix::net::RecvMsg`].

[`rustix::net::RecvMsg`]: https://docs.rs/rustix/1/rustix/net/struct.RecvMsg.html

The `flags` field of [`rustix::net::RecvMsg`] changed type from [`RecvFlags`]
to a new [`ReturnFlags`], since it supports a different set of flags.

[`rustix::net::RecvMsg`]: https://docs.rs/rustix/1/rustix/net/struct.RecvMsg.html
[`RecvFlags`]: https://docs.rs/rustix/1/rustix/net/struct.RecvFlags.html
[`ReturnFlags`]: https://docs.rs/rustix/1/rustix/net/struct.ReturnFlags.html

[`rustix::event::poll`]'s and [`rustix::event::epoll`]'s `timeout` argument
changed from a `c_int` where `-1` means no timeout and non-negative numbers
mean a timeout in milliseconds to an `Option<&Timespec>`. The [`Timespec`]'s
fields are `tv_sec` which holds seconds and `tv_nsec` which holds nanoseconds.

[`rustix::event::poll`]: https://docs.rs/rustix/1/rustix/event/fn.poll.html
[`rustix::event::epoll`]: https://docs.rs/rustix/1/rustix/event/epoll/index.html
[`Timespec`]: https://docs.rs/rustix/1/rustix/time/struct.Timespec.html

The timeout argument in [`rustix::thread::futex::wait`],
[`rustix::thread::futex::lock_pi`], [`rustix::thread::futex::wait_bitset`],
[`rustix::thread::futex::wait_requeue_pi`], and
[`rustix::thread::futex::lock_pi2`] changed from `Option<Timespec>` to
`Option<&Timespec>`, for consistency with the rest of rustix's API, and for
low-level efficiency, as it means the implementation doesn't need to make a
copy of the `Timespec` to take its address. An easy way to convert an
`Option<Timespec>` to an `Option<&Timespec> is to use [`Option::as_ref`].

[`rustix::thread::futex::wait`]: https://docs.rs/rustix/1/rustix/thread/futex/fn.wait.html
[`rustix::thread::futex::lock_pi`]: https://docs.rs/rustix/1/rustix/thread/futex/fn.lock_pi.html
[`rustix::thread::futex::wait_bitset`]: https://docs.rs/rustix/1/rustix/thread/futex/fn.wait_bitset.html
[`rustix::thread::futex::wait_requeue_pi`]: https://docs.rs/rustix/1/rustix/thread/futex/fn.wait_requeue_pi.html
[`rustix::thread::futex::lock_pi2`]: https://docs.rs/rustix/1/rustix/thread/futex/fn.lock_pi2.html
[`Option::as_ref`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html#method.as_ref

Functions in [`rustix::event::port`] are renamed to remove the redundant
`port_*` prefix.

[`rustix::event::port`]: https://docs.rs/rustix/1/x86_64-unknown-illumos/rustix/event/port/index.html

`rustix::fs::inotify::InotifyEvent` is renamed to
[`rustix::fs::inotify::Event`] to remove the redundant prefix.

[`rustix::fs::inotify::Event`]: https://docs.rs/rustix/1/rustix/fs/inotify/struct.Event.html

`rustix::fs::StatExt` is removed, and the timestamp fields `st_atime`,
`st_mtime`, and `st_ctime` of [`rustix::fs::Stat`] may now be accessed
directly. They are now signed instead of unsigned, so that they can represent
times before the epoch.

[`rustix::fs::Stat`]: https://docs.rs/rustix/1/rustix/fs/struct.Stat.html

`rustix::io::is_read_write` is removed, as it's higher-level functionality that
can be implemented in terms of lower-level rustix calls.

[`rustix::net::recv`] and [`rustix::net::recvfrom`] now include
the number of received bytes in their return types, as this number may differ
from the number of bytes written to the buffer when
[`rustix::net::RecvFlags::TRUNC`] is used.

[`rustix::net::recv`]: https://docs.rs/rustix/1/rustix/net/fn.recv.html
[`rustix::net::recvfrom`]: https://docs.rs/rustix/1/rustix/net/fn.recvfrom.html
[`rustix::net::RecvFlags::TRUNC`]: https://docs.rs/rustix/1/rustix/net/struct.RecvFlags.html#associatedconstant.TRUNC

[`rustix::process::Signal`] constants are now upper-cased; for example,
`Signal::Int` is now named [`Signal::INT`]. Also, `Signal` is no longer
directly convertible to `i32`; use [`Signal::as_raw`] instead.

[`rustix::process::Signal`]: https://docs.rs/rustix/1/rustix/process/struct.Signal.html
[`Signal::INT`]: https://docs.rs/rustix/1/rustix/process/struct.Signal.html#variant.Int
[`Signal::as_raw`]: https://docs.rs/rustix/1/rustix/process/struct.Signal.html#method.as_raw

`Signal::from_raw` is renamed to [`Signal::from_named_raw`].

[`Signal::from_named_raw`]: https://docs.rs/rustix/1/rustix/process/struct.Signal.html#method.from_named_raw

The associated constant `rustix::ioctl::Ioctl::OPCODE` is now replaced with an
associated method [`rustix::ioctl::Ioctl::opcode`], to support ioctls where the
opcode is computed rather than a constant.

[`rustix::ioctl::Ioctl::opcode`]: https://docs.rs/rustix/1/rustix/ioctl/trait.Ioctl.html#tymethod.opcode

The `ifindex` argument in
[`rustix::net::sockopt::set_ip_add_membership_with_ifindex`] and
[`rustix::net::sockopt::set_ip_drop_membership_with_ifindex`]
changed from `i32` to `u32`.

[`rustix::net::sockopt::set_ip_add_membership_with_ifindex`]: https://docs.rs/rustix/1/rustix/net/sockopt/fn.set_ip_add_membership_with_ifindex.html
[`rustix::net::sockopt::set_ip_drop_membership_with_ifindex`]: https://docs.rs/rustix/1/rustix/net/sockopt/fn.set_ip_drop_membership_with_ifindex.html

The `list` argument in [`rustix::fs::listxattr`], [`rustix::fs::flistxattr`],
and [`rustix::fs::llistxattr`] changed from `[c_char]`, which is `[i8]` on some
architectures, to `[u8]`.

[`rustix::fs::listxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.listxattr.html
[`rustix::fs::flistxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.flistxattr.html
[`rustix::fs::llistxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.llistxattr.html

On NetBSD, the nanoseconds fields of [`Stat`] have been renamed, for consistency
with other platforms:

| Old name       | New Name        |
| -------------- | --------------- |
| `st_atimensec` | `st_atime_nsec` |
| `st_mtimensec` | `st_mtime_nsec` |
| `st_ctimensec` | `st_ctime_nsec` |
| `st_birthtimensec` | `st_birthtime_nsec` |

[`Stat`]: https://docs.rs/rustix/1/x86_64-unknown-netbsd/rustix/fs/struct.Stat.html

[`rustix::mount::mount`]'s `data` argument is now an `Option`, so it can now
be used in place of `mount2`, and `mount2` is now removed.

[`rustix::mount::mount`]: https://docs.rs/rustix/1/rustix/mount/fn.mount.html

The [`rustix::net`] functions ending with `_v4`, `_v6`, `_unix` and `_xdp` have
been merged into a single function that accepts any address type.

Specifically, the following functions are removed:

  * `bind_any`, `bind_unix`, `bind_v4`, `bind_v6`, `bind_xdp` in favor of
    [`bind`],
  * `connect_any`, `connect_unix`, `connect_v4`, `connect_v6` in favor of
    [`connect`] (leaving address-less [`connect_unspec`]),
  * `sendmsg_v4`, `sendmsg_v6`, `sendmsg_unix`, `sendmsg_xdp`, `sendmsg_any` in
    favor of [`sendmsg_addr`] (leaving address-less [`sendmsg`]),
  * `sendto_any`, `sendto_v4`, `sendto_v6`, `sendto_unix`, `sendto_xdp` in
    favor of [`sendto`].

[`rustix::net`]: https://docs.rs/rustix/1/rustix/net/index.html
[`bind`]: https://docs.rs/rustix/1/rustix/net/fn.bind.html
[`connect`]: https://docs.rs/rustix/1/rustix/net/fn.connect.html
[`connect_unspec`]: https://docs.rs/rustix/1/rustix/net/fn.connect_unspec.html
[`sendmsg_addr`]: https://docs.rs/rustix/1/rustix/net/fn.sendmsg_addr.html
[`sendmsg`]: https://docs.rs/rustix/1/rustix/net/fn.sendmsg.html
[`sendto`]: https://docs.rs/rustix/1/rustix/net/fn.sendto.html

The `SocketAddrAny` enum has changed to a [`SocketAddrAny`] struct which can
contain any kind of socket address. It can be converted to and from the more
specific socket types using `From`/`Into`/`TryFrom`/`TryInto` conversions.

[`SocketAddrAny`]: https://docs.rs/rustix/1/rustix/net/struct.SocketAddrAny.html

The `len` parameter to [`rustix::fs::fadvise`] has changed from `u64` to
`Option<NonZeroU64>`, to reflect that zero is a special case meaning the
advice applies to the end of the file. To convert an arbitrary `u64` value to
`Option<NonZeroU64>`, use `NonZeroU64::new`.

[`rustix::fs::fadvise`]: https://docs.rs/rustix/1/rustix/fs/fn.fadvise.html

[`rustix::io_uring::io_uring_enter`] no longer has `arg` and `size` arguments
providing a raw `*mut c_void` and `usize` describing the argument value. To
pass argumentts, there are now additional functions, `io_uring_enter_sigmask`,
and `io_uring_enter_arg`, which take a [`KernelSigSet`] or an
`io_uring_getevents_arg`, respectively. These are more ergonomic, and provide
a better path to adding `IORING_ENTER_EXT_ARG_REG` support in the future.

[`rustix::io_uring::io_uring_enter`]: https://docs.rs/rustix/1/rustix/io_uring/fn.io_uring_enter.html
[`KernelSigSet`]: https://docs.rs/rustix/1/rustix/io_uring/struct.KernelSigSet.html

The [`sigmask`] and [`ts`] fields of [`rustix::io_uring::getevents_arg`]
changed from `u64` to [`rustix::io_uring::io_uring_ptr`], to better preserve
pointer provenance.

[`sigmask`]: https://docs.rs/rustix/1/rustix/io_uring/struct.io_uring_getevents_arg.html#structfield.sigmask
[`ts`]: https://docs.rs/rustix/1/rustix/io_uring/struct.io_uring_getevents_arg.html#structfield.ts
[`rustix::io_uring::getevents_arg`]: https://docs.rs/rustix/1/rustix/io_uring/struct.io_uring_getevents_arg.html
[`rustix::io_uring::io_uring_ptr`]: https://docs.rs/rustix/1/rustix/io_uring/struct.io_uring_ptr.html

The aliases for [`fcntl_dupfd_cloexec`], [`fcntl_getfd`], and [`fcntl_setfd`]
in `rustix::fs` are removed; these functions are just available in
[`rustix::io`] now.

[`fcntl_dupfd_cloexec`]: https://docs.rs/rustix/1/rustix/io/fn.fcntl_dupfd_cloexec.html
[`fcntl_getfd`]: https://docs.rs/rustix/1/rustix/io/fn.fcntl_getfd.html
[`fcntl_setfd`]: https://docs.rs/rustix/1/rustix/io/fn.fcntl_setfd.html
[`rustix::io`]: https://docs.rs/rustix/1/rustix/io/index.html

[`SocketAddrXdp`] no longer has a shared UMEM field. A new
[`SocketAddrXdpWithSharedUmem`] is added for the purpose of calling `bind` and
passing it an XDP address with a shared UMEM fd. And `SockaddrXdpFlags` is
renamed to [`SocketAddrXdpFlags`].

[`SocketAddrXdp`]: https://docs.rs/rustix/1/rustix/net/xdp/struct.SocketAddrXdp.html
[`SocketAddrXdpWithSharedUmem`]: https://docs.rs/rustix/1/rustix/net/xdp/struct.SocketAddrXdpWithSharedUmem.html
[`SocketAddrXdpFlags`]: https://docs.rs/rustix/1/rustix/net/xdp/struct.SocketAddrXdpFlags.html

[`rustix::io_uring::io_uring_setup`] is now unsafe, due its `io_uring_params`
argument optionally containing a raw file descriptor.

[`rustix::io_uring::io_uring_setup`]: https://docs.rs/rustix/1/rustix/io_uring/fn.io_uring_setup.html

The buffer for [`SendAncillaryBuffer`] and [`RecvAncillaryBuffer`] is now
a `[MaybeUninit<u8>]` instead of a `[u8]`.

[`SendAncillaryBuffer`]: https://docs.rs/rustix/1/rustix/net/struct.SendAncillaryBuffer.html
[`RecvAncillaryBuffer`]: https://docs.rs/rustix/1/rustix/net/struct.RecvAncillaryBuffer.html

[`read`], [`pread`], [`recv`], [`recvfrom`], [`getrandom`], [`readlinkat_raw`],
[`epoll::wait`], [`kevent`], [`port::getn`], [`getxattr`], [`lgetxattr`],
[`fgetxattr`], [`listxattr`], [`llistxattr`], and [`flistxattr`] now use the
new [`Buffer` trait].

This replaces `read_uninit`, `pread_uninit`, `recv_uninit`, `recvfrom_uninit`,
and `getrandom_uninit`, as the `Buffer` trait supports reading into
uninitialized slices.

`epoll::wait`, `kevent`, and `port::getn` previously took a `Vec` which they
implicitly cleared before results were appended. When passing a `Vec` to
`epoll::wait`, `kevent`, or `port::getn` using [`spare_capacity`], the `Vec` is
not cleared first. Consider clearing the vector before calling `epoll::wait`,
`kevent`, or `port::getn`, or consuming it using `.drain(..)` before reusing it.

[`read`]: https://docs.rs/rustix/1/rustix/io/fn.read.html
[`pread`]: https://docs.rs/rustix/1/rustix/io/fn.pread.html
[`recv`]: https://docs.rs/rustix/1/rustix/net/fn.recv.html
[`recvfrom`]: https://docs.rs/rustix/1/rustix/net/fn.recvfrom.html
[`getrandom`]: https://docs.rs/rustix/1/rustix/rand/fn.getrandom.html
[`readlinkat_raw`]: https://docs.rs/rustix/1/rustix/fs/fn.readlinkat_raw.html
[`epoll::wait`]: https://docs.rs/rustix/1/rustix/event/epoll/fn.wait.html
[`getxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.getxattr.html
[`lgetxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.lgetxattr.html
[`fgetxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.fgetxattr.html
[`listxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.listxattr.html
[`llistxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.llistxattr.html
[`flistxattr`]: https://docs.rs/rustix/1/rustix/fs/fn.flistxattr.html
[`kevent`]: https://docs.rs/rustix/1/x86_64-unknown-freebsd/rustix/event/kqueue/fn.kevent.html
[`port::getn`]: https://docs.rs/rustix/1/x86_64-unknown-illumos/rustix/event/port/fn.getn.html
[`Buffer` trait]: https://docs.rs/rustix/1/rustix/buffer/trait.Buffer.html
[`spare_capacity`]: https://docs.rs/rustix/1/rustix/buffer/fn.spare_capacity.html

The [`rustix::ioctl::Opcode`] type has changed from a struct to a raw integer
value, and the associated utilities are change to `const` functions. In place
of `ReadOpcode`, `WriteOpcode`, `ReadWriteOpcode`, and `NoneOpcode`, use the
`read`, `write`, `read_write`, and `none` const functions in the
[`ioctl::opcode`] module. For example, in place of this:
```rust
ioctl::Setter::<ioctl::ReadOpcode<b'U', 15, c_uint>, c_uint>::new(interface)
```
use this:
```rust
+ ioctl::Setter::<{ ioctl::opcode::read::<c_uint>(b'U', 15) }, c_uint>::new(interface)
```
.

In place of `BadOpcode`, use the opcode value directly.

[`rustix::ioctl::Opcode`]: https://docs.rs/rustix/1/rustix/ioctl/type.Opcode.html
[`ioctl::opcode`]: https://docs.rs/rustix/1/rustix/ioctl/opcode/index.html

[`rustix::event::port::getn`]'s `min_events` argument is now a `u32`, to
reflect the type in the underlying system API.

[`rustix::event::port::getn`]: https://docs.rs/rustix/1/x86_64-unknown-illumos/rustix/event/port/fn.getn.html

All explicitly deprecated functions and types have been removed. Their
deprecation messages will have identified alternatives.
