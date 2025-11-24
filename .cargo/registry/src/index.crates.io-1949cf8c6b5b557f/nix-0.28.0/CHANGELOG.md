All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/).

# Change Log

## [0.28.0] - 2024-02-24


### Added

- Added `mkdtemp` wrapper ([#1297](https://github.com/nix-rust/nix/pull/1297))
- Add associated constants `UTIME_OMIT` `UTIME_NOW` for `TimeSpec`
  ([#1879](https://github.com/nix-rust/nix/pull/1879))
- Added `EventFd` type. ([#1945](https://github.com/nix-rust/nix/pull/1945))
- - Added `impl From<Signal> for SigSet`.
  - Added `impl std::ops::BitOr for SigSet`.
  - Added `impl std::ops::BitOr for Signal`.
  - Added `impl std::ops::BitOr<Signal> for SigSet`

  ([#1959](https://github.com/nix-rust/nix/pull/1959))
- Added `TlsGetRecordType` control message type and corresponding enum for
  linux ([#2065](https://github.com/nix-rust/nix/pull/2065))
- Added `Ipv6HopLimit` to `::nix::sys::socket::ControlMessage` for Linux,
  MacOS, FreeBSD, DragonflyBSD, Android, iOS and Haiku.
  ([#2074](https://github.com/nix-rust/nix/pull/2074))
- Added `Icmp` and `IcmpV6` to `SockProtocol`
  ([#2103](https://github.com/nix-rust/nix/pull/2103))
- Added rfork support for FreeBSD in `unistd`
  ([#2121](https://github.com/nix-rust/nix/pull/2121))
- Added `MapFlags::map_hugetlb_with_size_log2` method for Linux targets
  ([#2125](https://github.com/nix-rust/nix/pull/2125))
- Added `mmap_anonymous` function
  ([#2127](https://github.com/nix-rust/nix/pull/2127))
- Added `mips32r6` and `mips64r6` support for signal, ioctl and ptrace
  ([#2138](https://github.com/nix-rust/nix/pull/2138))
- Added `F_GETPATH` FcntlFlags entry on Apple/NetBSD/DragonflyBSD for
  `::nix::fcntl`. ([#2142](https://github.com/nix-rust/nix/pull/2142))
- Added `F_KINFO` FcntlFlags entry on FreeBSD for `::nix::fcntl`.
  ([#2152](https://github.com/nix-rust/nix/pull/2152))
- Added `F_GETPATH_NOFIRMLINK` and `F_BARRIERFSYNC` FcntlFlags entry
  on Apple for `::nix::fcntl`.
  ([#2155](https://github.com/nix-rust/nix/pull/2155))
- Added newtype `Flock` to automatically unlock a held flock upon drop.
  Added `Flockable` trait to represent valid types for `Flock`.
  ([#2170](https://github.com/nix-rust/nix/pull/2170))
- Added `SetSockOpt` impls to enable Linux Kernel TLS on a TCP socket and to
  import TLS parameters. ([#2175](https://github.com/nix-rust/nix/pull/2175))
- - Added the `::nix::sys::socket::SocketTimestamp` enum for configuring the
  `TsClock` (a.k.a `SO_TS_CLOCK`) sockopt
  - Added FreeBSD's `ScmRealtime` and `ScmMonotonic` as new options in
  `::nix::sys::socket::ControlMessageOwned`

  ([#2187](https://github.com/nix-rust/nix/pull/2187))
- Added new fanotify API: wrappers for `fanotify_init` and `fanotify_mark`
  ([#2194](https://github.com/nix-rust/nix/pull/2194))
- Added `SpecialCharacterindices` support for haiku.
  ([#2195](https://github.com/nix-rust/nix/pull/2195))
- Added `sys::sendfile` support for solaris/illumos.
  ([#2198](https://github.com/nix-rust/nix/pull/2198))
- impl Display for InterfaceFlags
  ([#2206](https://github.com/nix-rust/nix/pull/2206))
- Added `sendfilev` in sys::sendfile for solarish
  ([#2207](https://github.com/nix-rust/nix/pull/2207))
- Added `fctrl::SealFlag::F_SEAL_FUTURE_WRITE`
  ([#2213](https://github.com/nix-rust/nix/pull/2213))
- Added `Ipv6MulticastHops` as socket option to set and read.
  ([#2234](https://github.com/nix-rust/nix/pull/2234))
- Enable `ControlMessageOwned::Ipv4RecvIf` and
  `ControlMessageOwned::Ipv4RecvDstAddr` for DragonFlyBSD
  ([#2240](https://github.com/nix-rust/nix/pull/2240))
- `ClockId::set_time()` and `time::clock_settime()` are now enabled on macOS
  ([#2241](https://github.com/nix-rust/nix/pull/2241))
- Added `IpBindAddressNoPort` sockopt to support `IP_BIND_ADDRESS_NO_PORT`
  available on linux. ([#2244](https://github.com/nix-rust/nix/pull/2244))
- Enable `MapFlags::map_hugetlb_with_size_log2` method for Android/Fuchsia
  ([#2245](https://github.com/nix-rust/nix/pull/2245))
- Added `TcpFastOpenConnect` sockopt to support `TCP_FASTOPEN_CONNECT`
  available on linux. ([#2247](https://github.com/nix-rust/nix/pull/2247))
- Add `reboot(2)` for OpenBSD/NetBSD
  ([#2251](https://github.com/nix-rust/nix/pull/2251))
- Added new `MemFdCreateFlag` constants to `sys::memfd` on Linux and Android
  related to hugetlbfs support.
  ([#2252](https://github.com/nix-rust/nix/pull/2252))
- Expose the inner fd of `Kqueue` through:

  * impl AsFd for Kqueue
  * impl From\<Kqueue\> for OwnedFd

  ([#2258](https://github.com/nix-rust/nix/pull/2258))
- Added `sys::eventfd` support on FreeBSD
  ([#2259](https://github.com/nix-rust/nix/pull/2259))
- Added `MmapFlags::MAP_FIXED` constant in `sys::mman` for netbsd and openbsd
  ([#2260](https://github.com/nix-rust/nix/pull/2260))
- Added the `SO_LISTENQLIMIT` sockopt.
  ([#2263](https://github.com/nix-rust/nix/pull/2263))
- Enable the `AT_EMPTY_PATH` flag for the `fchownat()` function
  ([#2267](https://github.com/nix-rust/nix/pull/2267))
- Add `AtFlags::AT_EMPTY_PATH` for FreeBSD and Hurd
  ([#2270](https://github.com/nix-rust/nix/pull/2270))
- Enable `OFlag::O_DIRECTORY for Solarish
  ([#2275](https://github.com/nix-rust/nix/pull/2275))
- Added the `Backlog` wrapper type for the `listen` call.
  ([#2276](https://github.com/nix-rust/nix/pull/2276))
- Add `clock_nanosleep()` ([#2277](https://github.com/nix-rust/nix/pull/2277))
- Enabled `O_DIRECT` in `fcntl::OFlags` for solarish
  ([#2278](https://github.com/nix-rust/nix/pull/2278))
- Added a new API sigsuspend.
  ([#2279](https://github.com/nix-rust/nix/pull/2279))
- - Added `errno::Errno::set` function
  - Added `errno::Errno::set_raw` function
  - Added `errno::Errno::last_raw` function
  - Added `errno::Errno::from_raw` function

  ([#2283](https://github.com/nix-rust/nix/pull/2283))
- Enable the `AT_EMPTY_PATH` flag for the `linkat()` function
  ([#2284](https://github.com/nix-rust/nix/pull/2284))
- Enable unistd::{sync, syncfs} for Android
  ([#2296](https://github.com/nix-rust/nix/pull/2296))

### Changed

- `poll` now takes `PollTimeout` replacing `libc::c_int`.
  ([#1876](https://github.com/nix-rust/nix/pull/1876))
- Deprecated `sys::eventfd::eventfd`.
  ([#1945](https://github.com/nix-rust/nix/pull/1945))
- `mmap`, `mmap_anonymous`, `munmap`, `mremap`, `madvise`, `msync`, `mprotect`,
  `munlock` and `mlock` updated to use `NonNull`.
  ([#2000](https://github.com/nix-rust/nix/pull/2000))
- `mmap` function now accepts `F` instead of `Option<F>`
  ([#2127](https://github.com/nix-rust/nix/pull/2127))
- `PollFd::new` now takes a `BorrowedFd` argument, with relaxed lifetime
    requirements relative to the previous version.
  ([#2134](https://github.com/nix-rust/nix/pull/2134))
- `FdSet::{insert, remove, contains}` now take `BorrowedFd` arguments, and have
  relaxed lifetime requirements relative to 0.27.1.
  ([#2136](https://github.com/nix-rust/nix/pull/2136))
- The following APIs now take an implementation of `AsFd` rather than a
    `RawFd`:

    - `unistd::tcgetpgrp`
    - `unistd::tcsetpgrp`
    - `unistd::fpathconf`
    - `unistd::ttyname`
    - `unistd::getpeereid` ([#2137](https://github.com/nix-rust/nix/pull/2137))
- Changed `openat()` and `Dir::openat()`, now take optional `dirfd`s
  ([#2139](https://github.com/nix-rust/nix/pull/2139))
- The MSRV is now 1.69 ([#2144](https://github.com/nix-rust/nix/pull/2144))
- Changed function `SockaddrIn::ip()` to return `net::Ipv4Addr` and refactored
  `SocketAddrV6::ip()` to be `const`
  ([#2151](https://github.com/nix-rust/nix/pull/2151))
- The following APIs now take optional `dirfd`s:

  - `readlinkat()`
  - `fstatat()`
  - `mknodat()`
  - `mkdirat()`
  - `execveat()`

  ([#2157](https://github.com/nix-rust/nix/pull/2157))
- `Epoll::wait` now takes `EpollTimeout` replacing `isize`.
  ([#2202](https://github.com/nix-rust/nix/pull/2202))
- - Deprecated `errno::errno()` function (use `Errno::last_raw()`)
  - Deprecated `errno::from_i32()` function (use `Errno::from_raw()`)
  - Deprecated `errno::Errno::from_i32()` function (use `Errno::from_raw()`)

  ([#2283](https://github.com/nix-rust/nix/pull/2283))

### Fixed

- Fix `SigSet` incorrect implementation of `Eq`, `PartialEq` and `Hash`
  ([#1946](https://github.com/nix-rust/nix/pull/1946))
- Fixed `::sys::socket::sockopt::IpMulticastTtl` by fixing the value of optlen
  passed to `libc::setsockopt` and added tests.
  ([#2072](https://github.com/nix-rust/nix/pull/2072))
- Fixed the function signature of `recvmmsg`, potentially causing UB
  ([#2119](https://github.com/nix-rust/nix/pull/2119))
- Fix `SignalFd::set_mask`.  In 0.27.0 it would actually close the file
  descriptor. ([#2141](https://github.com/nix-rust/nix/pull/2141))
- Fixed UnixAddr::new for haiku, it did not record the `sun_len` value as
  needed.
  Fixed `sys::socket::addr::from_raw_parts` and
  `sys::socket::Sockaddrlike::len` build for solaris.
  ([#2242](https://github.com/nix-rust/nix/pull/2242))
- Fixed solaris build globally.
  ([#2248](https://github.com/nix-rust/nix/pull/2248))
- Changed the `dup3` wrapper to perform a real call to `dup3` instead of
  emulating it via `dup2` and `fcntl` to get rid of race condition
  ([#2268](https://github.com/nix-rust/nix/pull/2268))
- Fixed `::unistd::Group::members` using read_unaligned to avoid crash on
  misaligned pointers ([#2311](https://github.com/nix-rust/nix/pull/2311))

### Removed

- The `FchownatFlags` type has been deprecated, please use `AtFlags` instead.
  ([#2267](https://github.com/nix-rust/nix/pull/2267))
- Removed the `dup3` wrapper on macOS, which was emulated via `dup2` and
  `fcntl` and could cause a race condition. The `dup3` system call is not
  supported on macOS. ([#2268](https://github.com/nix-rust/nix/pull/2268))
- The `LinkatFlags` type has been deprecated, please use `AtFlags` instead.
  ([#2284](https://github.com/nix-rust/nix/pull/2284))


## [0.27.1] - 2023-08-28

### Fixed

- Fixed generating the documentation on docs.rs.
  ([#2111](https://github.com/nix-rust/nix/pull/2111))

## [0.27.0] - 2023-08-28
### Added
- Added `AT_EACCESS` to `AtFlags` on all platforms but android
  ([#1995](https://github.com/nix-rust/nix/pull/1995))
- Add `PF_ROUTE` to `SockType` on macOS, iOS, all of the BSDs, Fuchsia, Haiku, Illumos.
  ([#1867](https://github.com/nix-rust/nix/pull/1867))
- Added `nix::ucontext` module on `aarch64-unknown-linux-gnu`.
  (#[1662](https://github.com/nix-rust/nix/pull/1662))
- Added `CanRaw` to `SockProtocol` and `CanBcm` as a separate `SocProtocol` constant.
  ([#1912](https://github.com/nix-rust/nix/pull/1912))
- Added `Generic` and `NFLOG` to `SockProtocol`.
  ([#2092](https://github.com/nix-rust/nix/pull/2092))
- Added `mq_timedreceive` to `::nix::mqueue`.
  ([#1966])(https://github.com/nix-rust/nix/pull/1966)
- Added `LocalPeerPid` to `nix::sys::socket::sockopt` for macOS. ([#1967](https://github.com/nix-rust/nix/pull/1967))
- Added `TFD_TIMER_CANCEL_ON_SET` to `::nix::sys::time::TimerSetTimeFlags` on Linux and Android.
  ([#2040](https://github.com/nix-rust/nix/pull/2040))
- Added `SOF_TIMESTAMPING_OPT_ID` and `SOF_TIMESTAMPING_OPT_TSONLY` to `nix::sys::socket::TimestampingFlag`.
  ([#2048](https://github.com/nix-rust/nix/pull/2048))
- Enabled socket timestamping options on Android. ([#2077](https://github.com/nix-rust/nix/pull/2077))
- Added vsock support for macOS ([#2056](https://github.com/nix-rust/nix/pull/2056))
- Added `SO_SETFIB` and `SO_USER_COOKIE` to `nix::sys::socket::sockopt` for FreeBSD.
  ([#2085](https://github.com/nix-rust/nix/pull/2085))
- Added `SO_RTABLE` for OpenBSD and `SO_ACCEPTFILTER` for FreeBSD/NetBSD to `nix::sys::socket::sockopt`.
  ([#2085](https://github.com/nix-rust/nix/pull/2085))
- Added `MSG_WAITFORONE` to `MsgFlags` on Android, Fuchsia, Linux, NetBSD,
  FreeBSD, OpenBSD, and Solaris.
  ([#2014](https://github.com/nix-rust/nix/pull/2014))
- Added `SO_TS_CLOCK` for FreeBSD to `nix::sys::socket::sockopt`.
  ([#2093](https://github.com/nix-rust/nix/pull/2093))
- Added support for prctl in Linux.
  (#[1550](https://github.com/nix-rust/nix/pull/1550))
- `nix::socket` and `nix::select` are now available on Redox.
  ([#2012](https://github.com/nix-rust/nix/pull/2012))
- Implemented AsFd, AsRawFd, FromRawFd, and IntoRawFd for `mqueue::MqdT`.
  ([#2097](https://github.com/nix-rust/nix/pull/2097))
- Add the ability to set `kevent_flags` on `SigEvent`.
  ([#1731](https://github.com/nix-rust/nix/pull/1731))

### Changed

- All Cargo features have been removed from the default set. Users will need to
  specify which features they depend on in their Cargo.toml.
  ([#2091](https://github.com/nix-rust/nix/pull/2091))
- Implemented I/O safety for many, but not all, of Nix's APIs.  Many public
  functions argument and return types have changed:
  | Original Type | New Type              |
  | ------------- | --------------------- |
  | AsRawFd       | AsFd                  |
  | RawFd         | BorrowedFd or OwnedFd |

  (#[1906](https://github.com/nix-rust/nix/pull/1906))
- Use I/O safety with `copy_file_range`, and expose it on FreeBSD.
  (#[1906](https://github.com/nix-rust/nix/pull/1906))
- The MSRV is now 1.65
  ([#1862](https://github.com/nix-rust/nix/pull/1862))
  ([#2104](https://github.com/nix-rust/nix/pull/2104))
- The epoll interface now uses a type.
  ([#1882](https://github.com/nix-rust/nix/pull/1882))
- With I/O-safe type applied in `pty::OpenptyResult` and `pty::ForkptyResult`,
  users no longer need to manually close the file descriptors in these types.
  ([#1921](https://github.com/nix-rust/nix/pull/1921))
- Refactored `name` parameter of `mq_open` and `mq_unlink` to be generic over
   `NixPath`.
  ([#2102](https://github.com/nix-rust/nix/pull/2102)).
- Made `clone` unsafe, like `fork`.
  ([#1993](https://github.com/nix-rust/nix/pull/1993))

### Removed

- `sys::event::{kevent, kevent_ts}` are deprecated in favor of
  `sys::kevent::Kqueue::kevent`, and `sys::event::kqueue` is deprecated in
  favor of `sys::kevent::Kqueue::new`.
  ([#1943](https://github.com/nix-rust/nix/pull/1943))
- Removed deprecated IoVec API.
  ([#1855](https://github.com/nix-rust/nix/pull/1855))
- Removed deprecated net APIs.
  ([#1861](https://github.com/nix-rust/nix/pull/1861))
- `nix::sys::signalfd::signalfd` is deprecated.  Use
  `nix::sys::signalfd::SignalFd` instead.
  ([#1938](https://github.com/nix-rust/nix/pull/1938))
- Removed `SigEvent` support on Fuchsia, where it was unsound.
  ([#2079](https://github.com/nix-rust/nix/pull/2079))
- Removed `flock` from `::nix::fcntl` on Solaris.
  ([#2082](https://github.com/nix-rust/nix/pull/2082))

## [0.26.3] - 2023-08-27

### Fixed
- Fix: send `ETH_P_ALL` in htons format
  ([#1925](https://github.com/nix-rust/nix/pull/1925))
- Fix: `recvmsg` now sets the length of the received `sockaddr_un` field
  correctly on Linux platforms. ([#2041](https://github.com/nix-rust/nix/pull/2041))
- Fix potentially invalid conversions in
  `SockaddrIn::from<std::net::SocketAddrV4>`,
  `SockaddrIn6::from<std::net::SockaddrV6>`, `IpMembershipRequest::new`, and
  `Ipv6MembershipRequest::new` with future Rust versions.
  ([#2061](https://github.com/nix-rust/nix/pull/2061))
- Fixed an incorrect lifetime returned from `recvmsg`.
  ([#2095](https://github.com/nix-rust/nix/pull/2095))

## [0.26.2] - 2023-01-18

### Fixed

- Fix `SockaddrIn6` bug that was swapping `flowinfo` and `scope_id` byte
  ordering.
  ([#1964](https://github.com/nix-rust/nix/pull/1964))

## [0.26.1] - 2022-11-29
### Fixed
- Fix UB with `sys::socket::sockopt::SockType` using `SOCK_PACKET`.
  ([#1821](https://github.com/nix-rust/nix/pull/1821))

## [0.26.0] - 2022-11-29
### Added

- Added `SockaddrStorage::{as_unix_addr, as_unix_addr_mut}`
  ([#1871](https://github.com/nix-rust/nix/pull/1871))
- Added `MntFlags` and `unmount` on all of the BSDs.
- Added `any()` and `all()` to `poll::PollFd`.
  ([#1877](https://github.com/nix-rust/nix/pull/1877))
- Add `MntFlags` and `unmount` on all of the BSDs.
  ([#1849](https://github.com/nix-rust/nix/pull/1849))
- Added a `Statfs::flags` method.
  ([#1849](https://github.com/nix-rust/nix/pull/1849))
- Added `NSFS_MAGIC` FsType on Linux and Android.
  ([#1829](https://github.com/nix-rust/nix/pull/1829))
- Added `sched_getcpu` on platforms that support it.
  ([#1825](https://github.com/nix-rust/nix/pull/1825))
- Added `sched_getaffinity` and `sched_setaffinity` on FreeBSD.
  ([#1804](https://github.com/nix-rust/nix/pull/1804))
- Added `line_discipline` field to `Termios` on Linux, Android and Haiku
  ([#1805](https://github.com/nix-rust/nix/pull/1805))
- Expose the memfd module on FreeBSD (memfd was added in FreeBSD 13)
  ([#1808](https://github.com/nix-rust/nix/pull/1808))
- Added `domainname` field of `UtsName` on Android and Linux
  ([#1817](https://github.com/nix-rust/nix/pull/1817))
- Re-export `RLIM_INFINITY` from `libc`
  ([#1831](https://github.com/nix-rust/nix/pull/1831))
- Added `syncfs(2)` on Linux
  ([#1833](https://github.com/nix-rust/nix/pull/1833))
- Added `faccessat(2)` on illumos
  ([#1841](https://github.com/nix-rust/nix/pull/1841))
- Added `eaccess()` on FreeBSD, DragonFly and Linux (glibc and musl).
  ([#1842](https://github.com/nix-rust/nix/pull/1842))
- Added `IP_TOS` `SO_PRIORITY` and `IPV6_TCLASS` sockopts for Linux
  ([#1853](https://github.com/nix-rust/nix/pull/1853))
- Added `new_unnamed` and `is_unnamed` for `UnixAddr` on Linux and Android.
  ([#1857](https://github.com/nix-rust/nix/pull/1857))
- Added `SockProtocol::Raw` for raw sockets
  ([#1848](https://github.com/nix-rust/nix/pull/1848))
- added `IP_MTU` (`IpMtu`) `IPPROTO_IP` sockopt on Linux and Android.
  ([#1865](https://github.com/nix-rust/nix/pull/1865))

### Changed

- The MSRV is now 1.56.1
  ([#1792](https://github.com/nix-rust/nix/pull/1792))
- The `addr` argument of `sys::mman::mmap` is now of type `Option<NonZeroUsize>`.
  ([#1870](https://github.com/nix-rust/nix/pull/1870))
- The `length` argument of `sys::mman::mmap` is now of type `NonZeroUsize`.
  ([#1873](https://github.com/nix-rust/nix/pull/1873))

### Fixed

- Fixed using `SockaddrStorage` to store a Unix-domain socket address on Linux.
  ([#1871](https://github.com/nix-rust/nix/pull/1871))
- Fix microsecond calculation for `TimeSpec`.
  ([#1801](https://github.com/nix-rust/nix/pull/1801))
- Fix `User::from_name` and `Group::from_name` panicking
  when given a name containing a nul.
  ([#1815](https://github.com/nix-rust/nix/pull/1815))
- Fix `User::from_uid` and `User::from_name` crash on Android platform.
  ([#1824](https://github.com/nix-rust/nix/pull/1824))
- Workaround XNU bug causing netmasks returned by `getifaddrs` to misbehave.
  ([#1788](https://github.com/nix-rust/nix/pull/1788))

### Removed

- Removed deprecated error constants and conversions.
  ([#1860](https://github.com/nix-rust/nix/pull/1860))

## [0.25.0] - 2022-08-13
### Added

- Added `faccessat`
  ([#1780](https://github.com/nix-rust/nix/pull/1780))
- Added `memfd` on Android.
  (#[1773](https://github.com/nix-rust/nix/pull/1773))
- Added `ETH_P_ALL` to `SockProtocol` enum
  (#[1768](https://github.com/nix-rust/nix/pull/1768))
- Added four non-standard Linux `SysconfVar` variants
  (#[1761](https://github.com/nix-rust/nix/pull/1761))
- Added const constructors for `TimeSpec` and `TimeVal`
  (#[1760](https://github.com/nix-rust/nix/pull/1760))
- Added `chflags`.
  (#[1758](https://github.com/nix-rust/nix/pull/1758))
- Added `aio_writev` and `aio_readv`.
  (#[1713](https://github.com/nix-rust/nix/pull/1713))
- impl `From<uid_t>` for `Uid` and `From<gid_t>` for `Gid`
  (#[1727](https://github.com/nix-rust/nix/pull/1727))
- impl `From<SockaddrIn>` for `std::net::SocketAddrV4` and
  impl `From<SockaddrIn6>` for `std::net::SocketAddrV6`.
  (#[1711](https://github.com/nix-rust/nix/pull/1711))
- Added support for the `x86_64-unknown-haiku` target.
  (#[1703](https://github.com/nix-rust/nix/pull/1703))
- Added `ptrace::read_user` and `ptrace::write_user` for Linux.
  (#[1697](https://github.com/nix-rust/nix/pull/1697))
- Added `getrusage` and helper types `UsageWho` and `Usage`
  (#[1747](https://github.com/nix-rust/nix/pull/1747))
- Added the `DontRoute` SockOpt
  (#[1752](https://github.com/nix-rust/nix/pull/1752))
- Added `signal::SigSet::from_sigset_t_unchecked()`.
  (#[1741](https://github.com/nix-rust/nix/pull/1741))
- Added the `Ipv4OrigDstAddr` sockopt and control message.
  (#[1772](https://github.com/nix-rust/nix/pull/1772))
- Added the `Ipv6OrigDstAddr` sockopt and control message.
  (#[1772](https://github.com/nix-rust/nix/pull/1772))
- Added the `Ipv4SendSrcAddr` control message.
  (#[1776](https://github.com/nix-rust/nix/pull/1776))

### Changed

- Reimplemented sendmmsg/recvmmsg to avoid allocations and with better API
  (#[1744](https://github.com/nix-rust/nix/pull/1744))

- Rewrote the aio module.  The new module:
  * Does more type checking at compile time rather than runtime.
  * Gives the caller control over whether and when to `Box` an aio operation.
  * Changes the type of the `priority` arguments to `i32`.
  * Changes the return type of `aio_return` to `usize`.
  (#[1713](https://github.com/nix-rust/nix/pull/1713))
- `nix::poll::ppoll`: `sigmask` parameter is now optional.
  (#[1739](https://github.com/nix-rust/nix/pull/1739))
- Changed `gethostname` to return an owned `OsString`.
  (#[1745](https://github.com/nix-rust/nix/pull/1745))
- `signal:SigSet` is now marked as `repr(transparent)`.
  (#[1741](https://github.com/nix-rust/nix/pull/1741))

### Removed

- Removed support for resubmitting partially complete `lio_listio` operations.
  It was too complicated, and didn't fit Nix's theme of zero-cost abstractions.
  Instead, it can be reimplemented downstream.
  (#[1713](https://github.com/nix-rust/nix/pull/1713))

## [0.24.2] - 2022-07-17
### Fixed

- Fixed buffer overflow in `nix::sys::socket::recvfrom`.
  (#[1763](https://github.com/nix-rust/nix/pull/1763))
- Enabled `SockaddrStorage::{as_link_addr, as_link_addr_mut}` for Linux-like
  operating systems.
  (#[1729](https://github.com/nix-rust/nix/pull/1729))
- Fixed `SockaddrLike::from_raw` implementations for `VsockAddr` and
  `SysControlAddr`.
  (#[1736](https://github.com/nix-rust/nix/pull/1736))

## [0.24.1] - 2022-04-22
### Fixed

- Fixed `UnixAddr::size` on Linux-based OSes.
  (#[1702](https://github.com/nix-rust/nix/pull/1702))

## [0.24.0] - 2022-04-21
### Added

- Added fine-grained features flags.  Most Nix functionality can now be
  conditionally enabled.  By default, all features are enabled.
  (#[1611](https://github.com/nix-rust/nix/pull/1611))
- Added statfs FS type magic constants for `target_os = "android"`
  and synced constants with libc v0.2.121.
  (#[1690](https://github.com/nix-rust/nix/pull/1690))
- Added `fexecve` on DragonFly.
  (#[1577](https://github.com/nix-rust/nix/pull/1577))
- `sys::uio::IoVec` is now `Send` and `Sync`
  (#[1582](https://github.com/nix-rust/nix/pull/1582))
- Added `EPOLLEXCLUSIVE` on Android.
  (#[1567](https://github.com/nix-rust/nix/pull/1567))
- Added `fdatasync` for FreeBSD, Fuchsia, NetBSD, and OpenBSD.
  (#[1581](https://github.com/nix-rust/nix/pull/1581))
- Added `sched_setaffinity` and `sched_getaffinity` on DragonFly.
  (#[1537](https://github.com/nix-rust/nix/pull/1537))
- Added `posix_fallocate` on DragonFly.
  (#[1621](https://github.com/nix-rust/nix/pull/1621))
- Added `SO_TIMESTAMPING` support
  (#[1547](https://github.com/nix-rust/nix/pull/1547))
- Added getter methods to `MqAttr` struct
  (#[1619](https://github.com/nix-rust/nix/pull/1619))
- Added the `TxTime` sockopt and control message.
  (#[1564](https://github.com/nix-rust/nix/pull/1564))
- Added POSIX per-process timer support
  (#[1622](https://github.com/nix-rust/nix/pull/1622))
- Added `sendfile` on DragonFly.
  (#[1615](https://github.com/nix-rust/nix/pull/1615))
- Added `UMOUNT_NOFOLLOW`, `FUSE_SUPER_MAGIC` on Linux.
  (#[1634](https://github.com/nix-rust/nix/pull/1634))
- Added `getresuid`, `setresuid`, `getresgid`, and `setresgid` on DragonFly, FreeBSD, and OpenBSD.
  (#[1628](https://github.com/nix-rust/nix/pull/1628))
- Added `MAP_FIXED_NOREPLACE` on Linux.
  (#[1636](https://github.com/nix-rust/nix/pull/1636))
- Added `fspacectl` on FreeBSD
  (#[1640](https://github.com/nix-rust/nix/pull/1640))
- Added `accept4` on DragonFly, Emscripten, Fuchsia, Illumos, and NetBSD.
  (#[1654](https://github.com/nix-rust/nix/pull/1654))
- Added `AsRawFd` implementation on `OwningIter`.
  (#[1563](https://github.com/nix-rust/nix/pull/1563))
- Added `process_vm_readv` and `process_vm_writev` on Android.
  (#[1557](https://github.com/nix-rust/nix/pull/1557))
- Added `nix::ucontext` module on s390x.
  (#[1662](https://github.com/nix-rust/nix/pull/1662))
- Implemented `Extend`, `FromIterator`, and `IntoIterator` for `SigSet` and
  added `SigSet::iter` and `SigSetIter`.
  (#[1553](https://github.com/nix-rust/nix/pull/1553))
- Added `ENOTRECOVERABLE` and `EOWNERDEAD` error codes on DragonFly.
  (#[1665](https://github.com/nix-rust/nix/pull/1665))
- Implemented `Read` and `Write` for `&PtyMaster`
  (#[1664](https://github.com/nix-rust/nix/pull/1664))
- Added `MSG_NOSIGNAL` for Android, Dragonfly, FreeBSD, Fuchsia, Haiku, Illumos, Linux, NetBSD, OpenBSD and Solaris.
  (#[1670](https://github.com/nix-rust/nix/pull/1670))
- Added `waitid`.
  (#[1584](https://github.com/nix-rust/nix/pull/1584))
- Added `Ipv6DontFrag` for android, iOS, linux and macOS.
- Added `IpDontFrag` for iOS, macOS.
  (#[1692](https://github.com/nix-rust/nix/pull/1692))

### Changed

- `mqueue` functions now operate on a distinct type, `nix::mqueue::MqdT`.
  Accessors take this type by reference, not by value.
  (#[1639](https://github.com/nix-rust/nix/pull/1639))
- Removed `SigSet::extend` in favor of `<SigSet as Extend<Signal>>::extend`.
  Because of this change, you now need `use std::iter::Extend` to call `extend`
  on a `SigSet`.
  (#[1553](https://github.com/nix-rust/nix/pull/1553))
- Removed the the `PATH_MAX` restriction from APIs accepting paths. Paths
  will now be allocated on the heap if they are too long. In addition, large
  instruction count improvements (~30x) were made to path handling.
  (#[1656](https://github.com/nix-rust/nix/pull/1656))
- Changed `getrlimit` and `setrlimit` to use `rlim_t` directly
  instead of `Option<rlim_t>`.
  (#[1668](https://github.com/nix-rust/nix/pull/1668))
- Deprecated `InetAddr` and `SockAddr` in favor of `SockaddrIn`, `SockaddrIn6`,
  and `SockaddrStorage`.
  (#[1684](https://github.com/nix-rust/nix/pull/1684))
- Deprecated `IpAddr`, `Ipv4Addr`, and `Ipv6Addr` in favor of their equivalents
  from the standard library.
  (#[1685](https://github.com/nix-rust/nix/pull/1685))
- `uname` now returns a `Result<UtsName>` instead of just a `UtsName` and
  ignoring failures from libc.  And getters on the `UtsName` struct now return
  an `&OsStr` instead of `&str`.
  (#[1672](https://github.com/nix-rust/nix/pull/1672))
- Replaced `IoVec` with `IoSlice` and `IoSliceMut`, and replaced `IoVec::from_slice` with
  `IoSlice::new`. (#[1643](https://github.com/nix-rust/nix/pull/1643))

### Fixed

- `InetAddr::from_std` now sets the `sin_len`/`sin6_len` fields on the BSDs.
  (#[1642](https://github.com/nix-rust/nix/pull/1642))
- Fixed a panic in `LinkAddr::addr`.  That function now returns an `Option`.
  (#[1675](https://github.com/nix-rust/nix/pull/1675))
  (#[1677](https://github.com/nix-rust/nix/pull/1677))

### Removed

- Removed public access to the inner fields of `NetlinkAddr`, `AlgAddr`,
  `SysControlAddr`, `LinkAddr`, and `VsockAddr`.
  (#[1614](https://github.com/nix-rust/nix/pull/1614))
- Removed `EventFlag::EV_SYSFLAG`.
  (#[1635](https://github.com/nix-rust/nix/pull/1635))

## [0.23.1] - 2021-12-16

### Changed

- Relaxed the bitflags requirement from 1.3.1 to 1.1.  This partially reverts
  #1492.  From now on, the MSRV is not guaranteed to work with all versions of
  all dependencies, just with some version of all dependencies.
  (#[1607](https://github.com/nix-rust/nix/pull/1607))

### Fixed

- Fixed soundness issues in `FdSet::insert`, `FdSet::remove`, and
  `FdSet::contains` involving file descriptors outside of the range
  `0..FD_SETSIZE`.
  (#[1575](https://github.com/nix-rust/nix/pull/1575))

## [0.23.0] - 2021-09-28
### Added

- Added the `LocalPeerCred` sockopt.
  (#[1482](https://github.com/nix-rust/nix/pull/1482))
- Added `TimeSpec::from_duration` and `TimeSpec::from_timespec`
  (#[1465](https://github.com/nix-rust/nix/pull/1465))
- Added `IPV6_V6ONLY` sockopt.
  (#[1470](https://github.com/nix-rust/nix/pull/1470))
- Added `impl From<User> for libc::passwd` trait implementation to convert a `User`
  into a `libc::passwd`. Consumes the `User` struct to give ownership over
  the member pointers.
  (#[1471](https://github.com/nix-rust/nix/pull/1471))
- Added `pthread_kill`.
  (#[1472](https://github.com/nix-rust/nix/pull/1472))
- Added `mknodat`.
  (#[1473](https://github.com/nix-rust/nix/pull/1473))
- Added `setrlimit` and `getrlimit`.
  (#[1302](https://github.com/nix-rust/nix/pull/1302))
- Added `ptrace::interrupt` method for platforms that support `PTRACE_INTERRUPT`
  (#[1422](https://github.com/nix-rust/nix/pull/1422))
- Added `IP6T_SO_ORIGINAL_DST` sockopt.
  (#[1490](https://github.com/nix-rust/nix/pull/1490))
- Added the `PTRACE_EVENT_STOP` variant to the `sys::ptrace::Event` enum
  (#[1335](https://github.com/nix-rust/nix/pull/1335))
- Exposed `SockAddr::from_raw_sockaddr`
  (#[1447](https://github.com/nix-rust/nix/pull/1447))
- Added `TcpRepair`
  (#[1503](https://github.com/nix-rust/nix/pull/1503))
- Enabled `pwritev` and `preadv` for more operating systems.
  (#[1511](https://github.com/nix-rust/nix/pull/1511))
- Added support for `TCP_MAXSEG` TCP Maximum Segment Size socket options
  (#[1292](https://github.com/nix-rust/nix/pull/1292))
- Added `Ipv4RecvErr` and `Ipv6RecvErr` sockopts and associated control messages.
  (#[1514](https://github.com/nix-rust/nix/pull/1514))
- Added `AsRawFd` implementation on `PollFd`.
  (#[1516](https://github.com/nix-rust/nix/pull/1516))
- Added `Ipv4Ttl` and `Ipv6Ttl` sockopts.
  (#[1515](https://github.com/nix-rust/nix/pull/1515))
- Added `MAP_EXCL`, `MAP_ALIGNED_SUPER`, and `MAP_CONCEAL` mmap flags, and
  exposed `MAP_ANONYMOUS` for all operating systems.
  (#[1522](https://github.com/nix-rust/nix/pull/1522))
  (#[1525](https://github.com/nix-rust/nix/pull/1525))
  (#[1531](https://github.com/nix-rust/nix/pull/1531))
  (#[1534](https://github.com/nix-rust/nix/pull/1534))
- Added read/write accessors for 'events' on `PollFd`.
  (#[1517](https://github.com/nix-rust/nix/pull/1517))

### Changed

- `FdSet::{contains, highest, fds}` no longer require a mutable reference.
  (#[1464](https://github.com/nix-rust/nix/pull/1464))
- `User::gecos` and corresponding `libc::passwd::pw_gecos` are supported on
  64-bit Android, change conditional compilation to include the field in
  64-bit Android builds
  (#[1471](https://github.com/nix-rust/nix/pull/1471))
- `eventfd`s are supported on Android, change conditional compilation to
  include `sys::eventfd::eventfd` and `sys::eventfd::EfdFlags`for Android
  builds.
  (#[1481](https://github.com/nix-rust/nix/pull/1481))
- Most enums that come from C, for example `Errno`, are now marked as
  `#[non_exhaustive]`.
  (#[1474](https://github.com/nix-rust/nix/pull/1474))
- Many more functions, mostly contructors, are now `const`.
  (#[1476](https://github.com/nix-rust/nix/pull/1476))
  (#[1492](https://github.com/nix-rust/nix/pull/1492))
- `sys::event::KEvent::filter` now returns a `Result` instead of being
  infalliable.  The only cases where it will now return an error are cases
  where it previously would've had undefined behavior.
  (#[1484](https://github.com/nix-rust/nix/pull/1484))
- Minimum supported Rust version is now 1.46.0.
  ([#1492](https://github.com/nix-rust/nix/pull/1492))
- Rework `UnixAddr` to encapsulate internals better in order to fix soundness
  issues. No longer allows creating a `UnixAddr` from a raw `sockaddr_un`.
  ([#1496](https://github.com/nix-rust/nix/pull/1496))
- Raised bitflags to 1.3.0 and the MSRV to 1.46.0.
  ([#1492](https://github.com/nix-rust/nix/pull/1492))

### Fixed

- `posix_fadvise` now returns errors in the conventional way, rather than as a
  non-zero value in `Ok()`.
  (#[1538](https://github.com/nix-rust/nix/pull/1538))
- Added more errno definitions for better backwards compatibility with
  Nix 0.21.0.
  (#[1467](https://github.com/nix-rust/nix/pull/1467))
- Fixed potential undefined behavior in `Signal::try_from` on some platforms.
  (#[1484](https://github.com/nix-rust/nix/pull/1484))
- Fixed buffer overflow in `unistd::getgrouplist`.
  (#[1545](https://github.com/nix-rust/nix/pull/1545))


### Removed

- Removed a couple of termios constants on redox that were never actually
  supported.
  (#[1483](https://github.com/nix-rust/nix/pull/1483))
- Removed `nix::sys::signal::NSIG`.  It was of dubious utility, and not correct
  for all platforms.
  (#[1484](https://github.com/nix-rust/nix/pull/1484))
- Removed support for 32-bit Apple targets, since they've been dropped by both
  Rustc and Xcode.
  (#[1492](https://github.com/nix-rust/nix/pull/1492))
- Deprecated `SockAddr/InetAddr::to_str` in favor of `ToString::to_string`
  (#[1495](https://github.com/nix-rust/nix/pull/1495))
- Removed `SigevNotify` on OpenBSD and Redox.
  (#[1511](https://github.com/nix-rust/nix/pull/1511))

## [0.22.3] - 22 January 2022
### Changed
- Relaxed the bitflags requirement from 1.3.1 to 1.1.  This partially reverts
  #1492.  From now on, the MSRV is not guaranteed to work with all versions of
  all dependencies, just with some version of all dependencies.
  (#[1607](https://github.com/nix-rust/nix/pull/1607))

## [0.22.2] - 28 September 2021
### Fixed
- Fixed buffer overflow in `unistd::getgrouplist`.
  (#[1545](https://github.com/nix-rust/nix/pull/1545))
- Added more errno definitions for better backwards compatibility with
  Nix 0.21.0.
  (#[1467](https://github.com/nix-rust/nix/pull/1467))

## [0.22.1] - 13 August 2021
### Fixed
- Locked bitflags to < 1.3.0 to fix the build with rust < 1.46.0.

### Removed
- Removed a couple of termios constants on redox that were never actually
  supported.
  (#[1483](https://github.com/nix-rust/nix/pull/1483))

## [0.22.0] - 9 July 2021
### Added
- Added `if_nameindex` (#[1445](https://github.com/nix-rust/nix/pull/1445))
- Added `nmount` for FreeBSD.
  (#[1453](https://github.com/nix-rust/nix/pull/1453))
- Added `IpFreebind` socket option (sockopt) on Linux, Fuchsia and Android.
  (#[1456](https://github.com/nix-rust/nix/pull/1456))
- Added `TcpUserTimeout` socket option (sockopt) on Linux and Fuchsia.
  (#[1457](https://github.com/nix-rust/nix/pull/1457))
- Added `renameat2` for Linux
  (#[1458](https://github.com/nix-rust/nix/pull/1458))
- Added `RxqOvfl` support on Linux, Fuchsia and Android.
  (#[1455](https://github.com/nix-rust/nix/pull/1455))

### Changed
- `ptsname_r` now returns a lossily-converted string in the event of bad UTF,
  just like `ptsname`.
  ([#1446](https://github.com/nix-rust/nix/pull/1446))
- Nix's error type is now a simple wrapper around the platform's Errno.  This
  means it is now `Into<std::io::Error>`.  It's also `Clone`, `Copy`, `Eq`, and
  has a small fixed size.  It also requires less typing.  For example, the old
  enum variant `nix::Error::Sys(nix::errno::Errno::EINVAL)` is now simply
  `nix::Error::EINVAL`.
  ([#1446](https://github.com/nix-rust/nix/pull/1446))

## [0.21.2] - 29 September 2021
### Fixed
- Fixed buffer overflow in `unistd::getgrouplist`.
  (#[1545](https://github.com/nix-rust/nix/pull/1545))

## [0.21.1] - 13 August 2021
### Fixed
- Locked bitflags to < 1.3.0 to fix the build with rust < 1.46.0.

### Removed
- Removed a couple of termios constants on redox that were never actually
  supported.
  (#[1483](https://github.com/nix-rust/nix/pull/1483))

## [0.21.0] - 31 May 2021
### Added
- Added `getresuid` and `getresgid`
  (#[1430](https://github.com/nix-rust/nix/pull/1430))
- Added TIMESTAMPNS support for linux
  (#[1402](https://github.com/nix-rust/nix/pull/1402))
- Added `sendfile64` (#[1439](https://github.com/nix-rust/nix/pull/1439))
- Added `MS_LAZYTIME` to `MsFlags`
  (#[1437](https://github.com/nix-rust/nix/pull/1437))

### Changed
- Made `forkpty` unsafe, like `fork`
  (#[1390](https://github.com/nix-rust/nix/pull/1390))
- Made `Uid`, `Gid` and `Pid` methods `from_raw` and `as_raw` a `const fn`
  (#[1429](https://github.com/nix-rust/nix/pull/1429))
- Made `Uid::is_root` a `const fn`
  (#[1429](https://github.com/nix-rust/nix/pull/1429))
- `AioCb` is now always pinned.  Once a `libc::aiocb` gets sent to the kernel,
  its address in memory must not change.  Nix now enforces that by using
  `std::pin`.  Most users won't need to change anything, except when using
  `aio_suspend`.  See that method's documentation for the new usage.
  (#[1440](https://github.com/nix-rust/nix/pull/1440))
- `LioCb` is now constructed using a distinct `LioCbBuilder` struct.  This
  avoids a soundness issue with the old `LioCb`.  Usage is similar but
  construction now uses the builder pattern.  See the documentation for
  details.
  (#[1440](https://github.com/nix-rust/nix/pull/1440))
- Minimum supported Rust version is now 1.41.0.
  ([#1440](https://github.com/nix-rust/nix/pull/1440))
- Errno aliases are now associated consts on `Errno`, instead of consts in the
  `errno` module.
  (#[1452](https://github.com/nix-rust/nix/pull/1452))

### Fixed
- Allow `sockaddr_ll` size, as reported by the Linux kernel, to be smaller then it's definition
  (#[1395](https://github.com/nix-rust/nix/pull/1395))
- Fix spurious errors using `sendmmsg` with multiple cmsgs
  (#[1414](https://github.com/nix-rust/nix/pull/1414))
- Added `Errno::EOPNOTSUPP` to FreeBSD, where it was missing.
  (#[1452](https://github.com/nix-rust/nix/pull/1452))

### Removed

- Removed `sys::socket::accept4` from Android arm because libc removed it in
  version 0.2.87.
  ([#1399](https://github.com/nix-rust/nix/pull/1399))
- `AioCb::from_boxed_slice` and `AioCb::from_boxed_mut_slice` have been
  removed.  They were useful with earlier versions of Rust, but should no
  longer be needed now that async/await are available.  `AioCb`s now work
  exclusively with borrowed buffers, not owned ones.
  (#[1440](https://github.com/nix-rust/nix/pull/1440))
- Removed some Errno values from platforms where they aren't actually defined.
  (#[1452](https://github.com/nix-rust/nix/pull/1452))

## [0.20.2] - 28 September 2021
### Fixed
- Fixed buffer overflow in `unistd::getgrouplist`.
  (#[1545](https://github.com/nix-rust/nix/pull/1545))

## [0.20.1] - 13 August 2021
### Fixed
- Locked bitflags to < 1.3.0 to fix the build with rust < 1.46.0.

### Removed
- Removed a couple of termios constants on redox that were never actually
  supported.
  (#[1483](https://github.com/nix-rust/nix/pull/1483))

## [0.20.0] - 20 February 2021
### Added

- Added a `passwd` field to `Group` (#[1338](https://github.com/nix-rust/nix/pull/1338))
- Added `mremap` (#[1306](https://github.com/nix-rust/nix/pull/1306))
- Added `personality` (#[1331](https://github.com/nix-rust/nix/pull/1331))
- Added limited Fuchsia support (#[1285](https://github.com/nix-rust/nix/pull/1285))
- Added `getpeereid` (#[1342](https://github.com/nix-rust/nix/pull/1342))
- Implemented `IntoIterator` for `Dir`
  (#[1333](https://github.com/nix-rust/nix/pull/1333)).

### Changed

- Minimum supported Rust version is now 1.40.0.
  ([#1356](https://github.com/nix-rust/nix/pull/1356))
- i686-apple-darwin has been demoted to Tier 2 support, because it's deprecated
  by Xcode.
  (#[1350](https://github.com/nix-rust/nix/pull/1350))
- Fixed calling `recvfrom` on an `AddrFamily::Packet` socket
  (#[1344](https://github.com/nix-rust/nix/pull/1344))

### Fixed
- `TimerFd` now closes the underlying fd on drop.
  ([#1381](https://github.com/nix-rust/nix/pull/1381))
- Define `*_MAGIC` filesystem constants on Linux s390x
  (#[1372](https://github.com/nix-rust/nix/pull/1372))
- mqueue, sysinfo, timespec, statfs, test_ptrace_syscall() on x32
  (#[1366](https://github.com/nix-rust/nix/pull/1366))

### Removed

- `Dir`, `SignalFd`, and `PtyMaster` are no longer `Clone`.
  (#[1382](https://github.com/nix-rust/nix/pull/1382))
- Removed `SockLevel`, which hasn't been used for a few years
  (#[1362](https://github.com/nix-rust/nix/pull/1362))
- Removed both `Copy` and `Clone` from `TimerFd`.
  ([#1381](https://github.com/nix-rust/nix/pull/1381))

## [0.19.1] - 28 November 2020
### Fixed
- Fixed bugs in `recvmmsg`.
  (#[1341](https://github.com/nix-rust/nix/pull/1341))

## [0.19.0] - 6 October 2020
### Added
- Added Netlink protocol families to the `SockProtocol` enum
  (#[1289](https://github.com/nix-rust/nix/pull/1289))
- Added `clock_gettime`, `clock_settime`, `clock_getres`,
  `clock_getcpuclockid` functions and `ClockId` struct.
  (#[1281](https://github.com/nix-rust/nix/pull/1281))
- Added wrapper functions for `PTRACE_SYSEMU` and `PTRACE_SYSEMU_SINGLESTEP`.
  (#[1300](https://github.com/nix-rust/nix/pull/1300))
- Add support for Vsock on Android rather than just Linux.
  (#[1301](https://github.com/nix-rust/nix/pull/1301))
- Added `TCP_KEEPCNT` and `TCP_KEEPINTVL` TCP keepalive options.
  (#[1283](https://github.com/nix-rust/nix/pull/1283))
### Changed
- Expose `SeekData` and `SeekHole` on all Linux targets
  (#[1284](https://github.com/nix-rust/nix/pull/1284))
- Changed unistd::{execv,execve,execvp,execvpe,fexecve,execveat} to take both `&[&CStr]` and `&[CString]` as its list argument(s).
  (#[1278](https://github.com/nix-rust/nix/pull/1278))
- Made `unistd::fork` an unsafe funtion, bringing it in line with [libstd's decision](https://github.com/rust-lang/rust/pull/58059).
  (#[1293](https://github.com/nix-rust/nix/pull/1293))

## [0.18.0] - 26 July 2020
### Added
- Added `fchown(2)` wrapper.
  (#[1257](https://github.com/nix-rust/nix/pull/1257))
- Added support on linux systems for `MAP_HUGE_`_`SIZE`_ family of flags.
  (#[1211](https://github.com/nix-rust/nix/pull/1211))
- Added support for `F_OFD_*` `fcntl` commands on Linux and Android.
  (#[1195](https://github.com/nix-rust/nix/pull/1195))
- Added `env::clearenv()`: calls `libc::clearenv` on platforms
  where it's available, and clears the environment of all variables
  via `std::env::vars` and `std::env::remove_var` on others.
  (#[1185](https://github.com/nix-rust/nix/pull/1185))
- `FsType` inner value made public.
  (#[1187](https://github.com/nix-rust/nix/pull/1187))
- Added `unistd::setfsuid` and `unistd::setfsgid` to set the user or group
  identity for filesystem checks per-thread.
  (#[1163](https://github.com/nix-rust/nix/pull/1163))
- Derived `Ord`, `PartialOrd` for `unistd::Pid` (#[1189](https://github.com/nix-rust/nix/pull/1189))
- Added `select::FdSet::fds` method to iterate over file descriptors in a set.
  ([#1207](https://github.com/nix-rust/nix/pull/1207))
- Added support for UDP generic segmentation offload (GSO) and generic
  receive offload (GRO) ([#1209](https://github.com/nix-rust/nix/pull/1209))
- Added support for `sendmmsg` and `recvmmsg` calls
  (#[1208](https://github.com/nix-rust/nix/pull/1208))
- Added support for `SCM_CREDS` messages (`UnixCredentials`) on FreeBSD/DragonFly
  (#[1216](https://github.com/nix-rust/nix/pull/1216))
- Added `BindToDevice` socket option (sockopt) on Linux
  (#[1233](https://github.com/nix-rust/nix/pull/1233))
- Added `EventFilter` bitflags for `EV_DISPATCH` and `EV_RECEIPT` on OpenBSD.
  (#[1252](https://github.com/nix-rust/nix/pull/1252))
- Added support for `Ipv4PacketInfo` and `Ipv6PacketInfo` to `ControlMessage`.
  (#[1222](https://github.com/nix-rust/nix/pull/1222))
- `CpuSet` and `UnixCredentials` now implement `Default`.
  (#[1244](https://github.com/nix-rust/nix/pull/1244))
- Added `unistd::ttyname`
  (#[1259](https://github.com/nix-rust/nix/pull/1259))
- Added support for `Ipv4PacketInfo` and `Ipv6PacketInfo` to `ControlMessage` for iOS and Android.
  (#[1265](https://github.com/nix-rust/nix/pull/1265))
- Added support for `TimerFd`.
  (#[1261](https://github.com/nix-rust/nix/pull/1261))

### Changed
- Changed `fallocate` return type from `c_int` to `()` (#[1201](https://github.com/nix-rust/nix/pull/1201))
- Enabled `sys::ptrace::setregs` and `sys::ptrace::getregs` on x86_64-unknown-linux-musl target
  (#[1198](https://github.com/nix-rust/nix/pull/1198))
- On Linux, `ptrace::write` is now an `unsafe` function. Caveat programmer.
  (#[1245](https://github.com/nix-rust/nix/pull/1245))
- `execv`, `execve`, `execvp` and `execveat` in `::nix::unistd` and `reboot` in
  `::nix::sys::reboot` now return `Result<Infallible>` instead of `Result<Void>` (#[1239](https://github.com/nix-rust/nix/pull/1239))
- `sys::socket::sockaddr_storage_to_addr` is no longer `unsafe`.  So is
  `offset_of!`.
- `sys::socket::sockaddr_storage_to_addr`, `offset_of!`, and `Errno::clear` are
  no longer `unsafe`.
- `SockAddr::as_ffi_pair`,`sys::socket::sockaddr_storage_to_addr`, `offset_of!`,
  and `Errno::clear` are no longer `unsafe`.
  (#[1244](https://github.com/nix-rust/nix/pull/1244))
- Several `Inotify` methods now take `self` by value instead of by reference
  (#[1244](https://github.com/nix-rust/nix/pull/1244))
- `nix::poll::ppoll`: `timeout` parameter is now optional, None is equivalent for infinite timeout.

### Fixed

- Fixed `getsockopt`.  The old code produced UB which triggers a panic with
  Rust 1.44.0.
  (#[1214](https://github.com/nix-rust/nix/pull/1214))

- Fixed a bug in nix::unistd that would result in an infinite loop
  when a group or user lookup required a buffer larger than
  16KB. (#[1198](https://github.com/nix-rust/nix/pull/1198))
- Fixed unaligned casting of `cmsg_data` to `af_alg_iv` (#[1206](https://github.com/nix-rust/nix/pull/1206))
- Fixed `readlink`/`readlinkat` when reading symlinks longer than `PATH_MAX` (#[1231](https://github.com/nix-rust/nix/pull/1231))
- `PollFd`, `EpollEvent`, `IpMembershipRequest`, `Ipv6MembershipRequest`,
  `TimeVal`, and `IoVec` are now `repr(transparent)`.  This is required for
  correctness's sake across all architectures and compilers, though now bugs
  have been reported so far.
  (#[1243](https://github.com/nix-rust/nix/pull/1243))
- Fixed unaligned pointer read in `Inotify::read_events`.
  (#[1244](https://github.com/nix-rust/nix/pull/1244))

### Removed

- Removed `sys::socket::addr::from_libc_sockaddr` from the public API.
  (#[1215](https://github.com/nix-rust/nix/pull/1215))
- Removed `sys::termios::{get_libc_termios, get_libc_termios_mut, update_wrapper`
  from the public API. These were previously hidden in the docs but still usable
  by downstream.
  (#[1235](https://github.com/nix-rust/nix/pull/1235))

- Nix no longer implements `NixPath` for `Option<P> where P: NixPath`.  Most
  Nix functions that accept `NixPath` arguments can't do anything useful with
  `None`.  The exceptions (`mount` and `quotactl_sync`) already take explicitly
  optional arguments.
  (#[1242](https://github.com/nix-rust/nix/pull/1242))

- Removed `unistd::daemon` and `unistd::pipe2` on OSX and ios
  (#[1255](https://github.com/nix-rust/nix/pull/1255))

- Removed `sys::event::FilterFlag::NOTE_EXIT_REPARENTED` and
  `sys::event::FilterFlag::NOTE_REAP` on OSX and ios.
  (#[1255](https://github.com/nix-rust/nix/pull/1255))

- Removed `sys::ptrace::ptrace` on Android and Linux.
  (#[1255](https://github.com/nix-rust/nix/pull/1255))

- Dropped support for powerpc64-unknown-linux-gnu
  (#[1266](https://github.com/nix-rust/nix/pull/1268))

## [0.17.0] - 3 February 2020
### Added
- Add `CLK_TCK` to `SysconfVar`
  (#[1177](https://github.com/nix-rust/nix/pull/1177))
### Removed
- Removed deprecated Error::description from error types
  (#[1175](https://github.com/nix-rust/nix/pull/1175))

## [0.16.1] - 23 December 2019
### Fixed

- Fixed the build for OpenBSD
  (#[1168](https://github.com/nix-rust/nix/pull/1168))

## [0.16.0] - 1 December 2019
### Added
- Added `ptrace::seize()`: similar to `attach()` on Linux
  but with better-defined semantics.
  (#[1154](https://github.com/nix-rust/nix/pull/1154))

- Added `Signal::as_str()`: returns signal name as `&'static str`
  (#[1138](https://github.com/nix-rust/nix/pull/1138))

- Added `posix_fallocate`.
  ([#1105](https://github.com/nix-rust/nix/pull/1105))

- Implemented `Default` for `FdSet`
  ([#1107](https://github.com/nix-rust/nix/pull/1107))

- Added `NixPath::is_empty`.
  ([#1107](https://github.com/nix-rust/nix/pull/1107))

- Added `mkfifoat`
  ([#1133](https://github.com/nix-rust/nix/pull/1133))

- Added `User::from_uid`, `User::from_name`, `User::from_gid` and
  `Group::from_name`,
  ([#1139](https://github.com/nix-rust/nix/pull/1139))

- Added `linkat`
  ([#1101](https://github.com/nix-rust/nix/pull/1101))

- Added `sched_getaffinity`.
  ([#1148](https://github.com/nix-rust/nix/pull/1148))

- Added optional `Signal` argument to `ptrace::{detach, syscall}` for signal
  injection. ([#1083](https://github.com/nix-rust/nix/pull/1083))

### Changed
- `sys::termios::BaudRate` now implements `TryFrom<speed_t>` instead of
  `From<speed_t>`.  The old `From` implementation would panic on failure.
  ([#1159](https://github.com/nix-rust/nix/pull/1159))

- `sys::socket::ControlMessage::ScmCredentials` and
  `sys::socket::ControlMessageOwned::ScmCredentials` now wrap `UnixCredentials`
  rather than `libc::ucred`.
  ([#1160](https://github.com/nix-rust/nix/pull/1160))

- `sys::socket::recvmsg` now takes a plain `Vec` instead of a `CmsgBuffer`
  implementor.  If you were already using `cmsg_space!`, then you needn't worry.
  ([#1156](https://github.com/nix-rust/nix/pull/1156))

- `sys::socket::recvfrom` now returns
  `Result<(usize, Option<SockAddr>)>` instead of `Result<(usize, SockAddr)>`.
  ([#1145](https://github.com/nix-rust/nix/pull/1145))

- `Signal::from_c_int` has been replaced by `Signal::try_from`
  ([#1113](https://github.com/nix-rust/nix/pull/1113))

- Changed `readlink` and `readlinkat` to return `OsString`
  ([#1109](https://github.com/nix-rust/nix/pull/1109))

  ```rust
  # use nix::fcntl::{readlink, readlinkat};
  // the buffer argument of `readlink` and `readlinkat` has been removed,
  // and the return value is now an owned type (`OsString`).
  // Existing code can be updated by removing the buffer argument
  // and removing any clone or similar operation on the output

  // old code `readlink(&path, &mut buf)` can be replaced with the following
  let _: OsString = readlink(&path);

  // old code `readlinkat(dirfd, &path, &mut buf)` can be replaced with the following
  let _: OsString = readlinkat(dirfd, &path);
  ```

- Minimum supported Rust version is now 1.36.0.
  ([#1108](https://github.com/nix-rust/nix/pull/1108))

- `Ipv4Addr::octets`, `Ipv4Addr::to_std`, `Error::as_errno`,
  `ForkResult::is_child`, `ForkResult::is_parent`, `Gid::as_raw`,
  `Uid::is_root`, `Uid::as_raw`, `Pid::as_raw`, and `PollFd::revents` now take
  `self` by value.
  ([#1107](https://github.com/nix-rust/nix/pull/1107))

- Type `&CString` for parameters of `exec(v|ve|vp|vpe|veat)` are changed to `&CStr`.
  ([#1121](https://github.com/nix-rust/nix/pull/1121))

### Fixed
- Fix length of abstract socket addresses
  ([#1120](https://github.com/nix-rust/nix/pull/1120))

- Fix initialization of msghdr in recvmsg/sendmsg when built with musl
  ([#1136](https://github.com/nix-rust/nix/pull/1136))

### Removed
- Remove the deprecated `CmsgSpace`.
  ([#1156](https://github.com/nix-rust/nix/pull/1156))

## [0.15.0] - 10 August 2019
### Added
- Added `MSG_WAITALL` to `MsgFlags` in `sys::socket`.
  ([#1079](https://github.com/nix-rust/nix/pull/1079))
- Implemented `Clone`, `Copy`, `Debug`, `Eq`, `Hash`, and `PartialEq` for most
  types that support them. ([#1035](https://github.com/nix-rust/nix/pull/1035))
- Added `copy_file_range` wrapper
  ([#1069](https://github.com/nix-rust/nix/pull/1069))
- Add `mkdirat`.
  ([#1084](https://github.com/nix-rust/nix/pull/1084))
- Add `posix_fadvise`.
  ([#1089](https://github.com/nix-rust/nix/pull/1089))
- Added `AF_VSOCK` to `AddressFamily`.
  ([#1091](https://github.com/nix-rust/nix/pull/1091))
- Add `unlinkat`
  ([#1058](https://github.com/nix-rust/nix/pull/1058))
- Add `renameat`.
  ([#1097](https://github.com/nix-rust/nix/pull/1097))

### Changed
- Support for `ifaddrs` now present when building for Android.
  ([#1077](https://github.com/nix-rust/nix/pull/1077))
- Minimum supported Rust version is now 1.31.0
  ([#1035](https://github.com/nix-rust/nix/pull/1035))
  ([#1095](https://github.com/nix-rust/nix/pull/1095))
- Now functions `statfs()` and `fstatfs()` return result with `Statfs` wrapper
  ([#928](https://github.com/nix-rust/nix/pull/928))

### Fixed
- Enabled `sched_yield` for all nix hosts.
  ([#1090](https://github.com/nix-rust/nix/pull/1090))

## [0.14.1] - 2019-06-06
### Added
- Macros exported by `nix` may now be imported via `use` on the Rust 2018
  edition without importing helper macros on Linux targets.
  ([#1066](https://github.com/nix-rust/nix/pull/1066))

  For example, in Rust 2018, the `ioctl_read_bad!` macro can now be imported
  without importing the `convert_ioctl_res!` macro.

  ```rust
  use nix::ioctl_read_bad;

  ioctl_read_bad!(tcgets, libc::TCGETS, libc::termios);
  ```

### Changed
- Changed some public types from reexports of libc types like `uint32_t` to the
  native equivalents like `u32.`
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))

### Fixed
- Fix the build on Android and Linux/mips with recent versions of libc.
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))

## [0.14.0] - 2019-05-21
### Added
- Add IP_RECVIF & IP_RECVDSTADDR. Enable IP_PKTINFO and IP6_PKTINFO on netbsd/openbsd.
  ([#1002](https://github.com/nix-rust/nix/pull/1002))
- Added `inotify_init1`, `inotify_add_watch` and `inotify_rm_watch` wrappers for
  Android and Linux. ([#1016](https://github.com/nix-rust/nix/pull/1016))
- Add `ALG_SET_IV`, `ALG_SET_OP` and `ALG_SET_AEAD_ASSOCLEN` control messages and `AF_ALG`
  socket types on Linux and Android ([#1031](https://github.com/nix-rust/nix/pull/1031))
- Add killpg
  ([#1034](https://github.com/nix-rust/nix/pull/1034))
- Added ENOTSUP errno support for Linux and Android.
  ([#969](https://github.com/nix-rust/nix/pull/969))
- Add several errno constants from OpenBSD 6.2
  ([#1036](https://github.com/nix-rust/nix/pull/1036))
- Added `from_std` and `to_std` methods for `sys::socket::IpAddr`
  ([#1043](https://github.com/nix-rust/nix/pull/1043))
- Added `nix::unistd:seteuid` and `nix::unistd::setegid` for those platforms that do
  not support `setresuid` nor `setresgid` respectively.
  ([#1044](https://github.com/nix-rust/nix/pull/1044))
- Added a `access` wrapper
  ([#1045](https://github.com/nix-rust/nix/pull/1045))
- Add `forkpty`
  ([#1042](https://github.com/nix-rust/nix/pull/1042))
- Add `sched_yield`
  ([#1050](https://github.com/nix-rust/nix/pull/1050))

### Changed
- `PollFd` event flags renamed to `PollFlags` ([#1024](https://github.com/nix-rust/nix/pull/1024/))
- `recvmsg` now returns an Iterator over `ControlMessageOwned` objects rather
  than `ControlMessage` objects.  This is sadly not backwards-compatible.  Fix
  code like this:
  ```rust
  if let ControlMessage::ScmRights(&fds) = cmsg {
  ```

  By replacing it with code like this:
  ```rust
  if let ControlMessageOwned::ScmRights(fds) = cmsg {
  ```
  ([#1020](https://github.com/nix-rust/nix/pull/1020))
- Replaced `CmsgSpace` with the `cmsg_space` macro.
  ([#1020](https://github.com/nix-rust/nix/pull/1020))

### Fixed
- Fixed multiple bugs when using `sendmsg` and `recvmsg` with ancillary control messages
  ([#1020](https://github.com/nix-rust/nix/pull/1020))
- Macros exported by `nix` may now be imported via `use` on the Rust 2018
  edition without importing helper macros for BSD targets.
  ([#1041](https://github.com/nix-rust/nix/pull/1041))

  For example, in Rust 2018, the `ioctl_read_bad!` macro can now be imported
  without importing the `convert_ioctl_res!` macro.

  ```rust
  use nix::ioctl_read_bad;

  ioctl_read_bad!(tcgets, libc::TCGETS, libc::termios);
  ```

### Removed
- `Daemon`, `NOTE_REAP`, and `NOTE_EXIT_REPARENTED` are now deprecated on OSX
  and iOS.
  ([#1033](https://github.com/nix-rust/nix/pull/1033))
- `PTRACE_GETREGS`, `PTRACE_SETREGS`, `PTRACE_GETFPREGS`, and
  `PTRACE_SETFPREGS` have been removed from some platforms where they never
  should've been defined in the first place.
  ([#1055](https://github.com/nix-rust/nix/pull/1055))

## [0.13.1] - 2019-06-10
### Changed
- Changed some public types from reexports of libc types like `uint32_t` to the
  native equivalents like `u32.`
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))

### Fixed
- Fix the build on Android and Linux/mips with recent versions of libc.
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))
- Fixed build on Linux/arm and Linux/s390x with the latest Rust libc
  ([52102cb](https://github.com/nix-rust/nix/commit/52102cb76398c4dfb9ea141b98c5b01a2e050973))

### Removed
- `Daemon`, `NOTE_REAP`, and `NOTE_EXIT_REPARENTED` are now deprecated on OSX
  and iOS.
  ([#1033](https://github.com/nix-rust/nix/pull/1033))

## [0.13.0] - 2019-01-15
### Added
- Added PKTINFO(V4) & V6PKTINFO cmsg support - Android/FreeBSD/iOS/Linux/MacOS.
  ([#990](https://github.com/nix-rust/nix/pull/990))
- Added support of CString type in `setsockopt`.
  ([#972](https://github.com/nix-rust/nix/pull/972))
- Added option `TCP_CONGESTION` in `setsockopt`.
  ([#972](https://github.com/nix-rust/nix/pull/972))
- Added `symlinkat` wrapper.
  ([#997](https://github.com/nix-rust/nix/pull/997))
- Added `ptrace::{getregs, setregs}`.
  ([#1010](https://github.com/nix-rust/nix/pull/1010))
- Added `nix::sys::signal::signal`.
  ([#817](https://github.com/nix-rust/nix/pull/817))
- Added an `mprotect` wrapper.
  ([#991](https://github.com/nix-rust/nix/pull/991))

### Fixed
- `lutimes` never worked on OpenBSD as it is not implemented on OpenBSD. It has
  been removed. ([#1000](https://github.com/nix-rust/nix/pull/1000))
- `fexecve` never worked on NetBSD or on OpenBSD as it is not implemented on
  either OS. It has been removed. ([#1000](https://github.com/nix-rust/nix/pull/1000))

## [0.12.1] 2019-06-08
### Changed
- Changed some public types from reexports of libc types like `uint32_t` to the
  native equivalents like `u32.`
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))

### Fixed
- Fix the build on Android and Linux/mips with recent versions of libc.
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))
- Fixed build on Linux/arm and Linux/s390x with the latest Rust libc
  ([52102cb](https://github.com/nix-rust/nix/commit/52102cb76398c4dfb9ea141b98c5b01a2e050973))

### Removed
- `fexecve` never worked on NetBSD or on OpenBSD as it is not implemented on
  either OS. It has been removed. ([#1000](https://github.com/nix-rust/nix/pull/1000))
- `Daemon`, `NOTE_REAP`, and `NOTE_EXIT_REPARENTED` are now deprecated on OSX
  and iOS.
  ([#1033](https://github.com/nix-rust/nix/pull/1033))

## [0.12.0] 2018-11-28

### Added
- Added `FromStr` and `Display` impls for `nix::sys::Signal`
  ([#884](https://github.com/nix-rust/nix/pull/884))
- Added a `sync` wrapper.
  ([#961](https://github.com/nix-rust/nix/pull/961))
- Added a `sysinfo` wrapper.
  ([#922](https://github.com/nix-rust/nix/pull/922))
- Support the `SO_PEERCRED` socket option and the `UnixCredentials` type on all Linux and Android targets.
  ([#921](https://github.com/nix-rust/nix/pull/921))
- Added support for `SCM_CREDENTIALS`, allowing to send process credentials over Unix sockets.
  ([#923](https://github.com/nix-rust/nix/pull/923))
- Added a `dir` module for reading directories (wraps `fdopendir`, `readdir`, and `rewinddir`).
  ([#916](https://github.com/nix-rust/nix/pull/916))
- Added `kmod` module that allows loading and unloading kernel modules on Linux.
  ([#930](https://github.com/nix-rust/nix/pull/930))
- Added `futimens` and `utimesat` wrappers ([#944](https://github.com/nix-rust/nix/pull/944)),
  an `lutimes` wrapper ([#967](https://github.com/nix-rust/nix/pull/967)),
  and a `utimes` wrapper ([#946](https://github.com/nix-rust/nix/pull/946)).
- Added `AF_UNSPEC` wrapper to `AddressFamily` ([#948](https://github.com/nix-rust/nix/pull/948))
- Added the `mode_t` public alias within `sys::stat`.
  ([#954](https://github.com/nix-rust/nix/pull/954))
- Added a `truncate` wrapper.
  ([#956](https://github.com/nix-rust/nix/pull/956))
- Added a `fchownat` wrapper.
  ([#955](https://github.com/nix-rust/nix/pull/955))
- Added support for `ptrace` on BSD operating systems ([#949](https://github.com/nix-rust/nix/pull/949))
- Added `ptrace` functions for reads and writes to tracee memory and ptrace kill
  ([#949](https://github.com/nix-rust/nix/pull/949)) ([#958](https://github.com/nix-rust/nix/pull/958))
- Added a `acct` wrapper module for enabling and disabling process accounting
  ([#952](https://github.com/nix-rust/nix/pull/952))
- Added the `time_t` and `suseconds_t` public aliases within `sys::time`.
  ([#968](https://github.com/nix-rust/nix/pull/968))
- Added `unistd::execvpe` for Haiku, Linux and OpenBSD
  ([#975](https://github.com/nix-rust/nix/pull/975))
- Added `Error::as_errno`.
  ([#977](https://github.com/nix-rust/nix/pull/977))

### Changed
- Increased required Rust version to 1.24.1
  ([#900](https://github.com/nix-rust/nix/pull/900))
  ([#966](https://github.com/nix-rust/nix/pull/966))

### Fixed
- Made `preadv` take immutable slice of IoVec.
  ([#914](https://github.com/nix-rust/nix/pull/914))
- Fixed passing multiple file descriptors over Unix Sockets.
  ([#918](https://github.com/nix-rust/nix/pull/918))

## [0.11.1] 2019-06-06
### Changed
- Changed some public types from reexports of libc types like `uint32_t` to the
  native equivalents like `u32.`
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))

### Fixed
- Fix the build on Android and Linux/mips with recent versions of libc.
  ([#1072](https://github.com/nix-rust/nix/pull/1072/commits))
- Fixed build on Linux/arm and Linux/s390x with the latest Rust libc
  ([52102cb](https://github.com/nix-rust/nix/commit/52102cb76398c4dfb9ea141b98c5b01a2e050973))

### Removed
- `fexecve` never worked on NetBSD or on OpenBSD as it is not implemented on
  either OS. It has been removed. ([#1000](https://github.com/nix-rust/nix/pull/1000))
- `Daemon`, `NOTE_REAP`, and `NOTE_EXIT_REPARENTED` are now deprecated on OSX
  and iOS.
  ([#1033](https://github.com/nix-rust/nix/pull/1033))

## [0.11.0] 2018-06-01

### Added
- Added `sendfile` on FreeBSD and Darwin.
  ([#901](https://github.com/nix-rust/nix/pull/901))
- Added `pselect`
  ([#894](https://github.com/nix-rust/nix/pull/894))
- Exposed `preadv` and `pwritev` on the BSDs.
  ([#883](https://github.com/nix-rust/nix/pull/883))
- Added `mlockall` and `munlockall`
  ([#876](https://github.com/nix-rust/nix/pull/876))
- Added `SO_MARK` on Linux.
  ([#873](https://github.com/nix-rust/nix/pull/873))
- Added safe support for nearly any buffer type in the `sys::aio` module.
  ([#872](https://github.com/nix-rust/nix/pull/872))
- Added `sys::aio::LioCb` as a wrapper for `libc::lio_listio`.
  ([#872](https://github.com/nix-rust/nix/pull/872))
- Added `unistd::getsid`
  ([#850](https://github.com/nix-rust/nix/pull/850))
- Added `alarm`. ([#830](https://github.com/nix-rust/nix/pull/830))
- Added interface flags `IFF_NO_PI, IFF_TUN, IFF_TAP` on linux-like systems.
  ([#853](https://github.com/nix-rust/nix/pull/853))
- Added `statvfs` module to all MacOS and Linux architectures.
  ([#832](https://github.com/nix-rust/nix/pull/832))
- Added `EVFILT_EMPTY`, `EVFILT_PROCDESC`, and `EVFILT_SENDFILE` on FreeBSD.
  ([#825](https://github.com/nix-rust/nix/pull/825))
- Exposed `termios::cfmakesane` on FreeBSD.
  ([#825](https://github.com/nix-rust/nix/pull/825))
- Exposed `MSG_CMSG_CLOEXEC` on *BSD.
  ([#825](https://github.com/nix-rust/nix/pull/825))
- Added `fchmod`, `fchmodat`.
  ([#857](https://github.com/nix-rust/nix/pull/857))
- Added `request_code_write_int!` on FreeBSD/DragonFlyBSD
  ([#833](https://github.com/nix-rust/nix/pull/833))

### Changed
- `Display` and `Debug` for `SysControlAddr` now includes all fields.
  ([#837](https://github.com/nix-rust/nix/pull/837))
- `ioctl!` has been replaced with a family of `ioctl_*!` macros.
  ([#833](https://github.com/nix-rust/nix/pull/833))
- `io!`, `ior!`, `iow!`, and `iorw!` has been renamed to `request_code_none!`, `request_code_read!`,
  `request_code_write!`, and `request_code_readwrite!` respectively. These have also now been exposed
  in the documentation.
  ([#833](https://github.com/nix-rust/nix/pull/833))
- Enabled more `ptrace::Request` definitions for uncommon Linux platforms
  ([#892](https://github.com/nix-rust/nix/pull/892))
- Emulation of `FD_CLOEXEC` and `O_NONBLOCK` was removed from `socket()`, `accept4()`, and
  `socketpair()`.
  ([#907](https://github.com/nix-rust/nix/pull/907))

### Fixed
- Fixed possible panics when using `SigAction::flags` on Linux
  ([#869](https://github.com/nix-rust/nix/pull/869))
- Properly exposed 460800 and 921600 baud rates on NetBSD
  ([#837](https://github.com/nix-rust/nix/pull/837))
- Fixed `ioctl_write_int!` on FreeBSD/DragonFlyBSD
  ([#833](https://github.com/nix-rust/nix/pull/833))
- `ioctl_write_int!` now properly supports passing a `c_ulong` as the parameter on Linux non-musl targets
  ([#833](https://github.com/nix-rust/nix/pull/833))

### Removed
- Removed explicit support for the `bytes` crate from the `sys::aio` module.
  See `sys::aio::AioCb::from_boxed_slice` examples for alternatives.
  ([#872](https://github.com/nix-rust/nix/pull/872))
- Removed `sys::aio::lio_listio`.  Use `sys::aio::LioCb::listio` instead.
  ([#872](https://github.com/nix-rust/nix/pull/872))
- Removed emulated `accept4()` from macos, ios, and netbsd targets
  ([#907](https://github.com/nix-rust/nix/pull/907))
- Removed `IFF_NOTRAILERS` on OpenBSD, as it has been removed in OpenBSD 6.3
  ([#893](https://github.com/nix-rust/nix/pull/893))

## [0.10.0] 2018-01-26

### Added
- Added specialized wrapper: `sys::ptrace::step`
  ([#852](https://github.com/nix-rust/nix/pull/852))
- Added `AioCb::from_ptr` and `AioCb::from_mut_ptr`
  ([#820](https://github.com/nix-rust/nix/pull/820))
- Added specialized wrappers: `sys::ptrace::{traceme, syscall, cont, attach}`. Using the matching routines
  with `sys::ptrace::ptrace` is now deprecated.
- Added `nix::poll` module for all platforms
  ([#672](https://github.com/nix-rust/nix/pull/672))
- Added `nix::ppoll` function for FreeBSD and DragonFly
  ([#672](https://github.com/nix-rust/nix/pull/672))
- Added protocol families in `AddressFamily` enum.
  ([#647](https://github.com/nix-rust/nix/pull/647))
- Added the `pid()` method to `WaitStatus` for extracting the PID.
  ([#722](https://github.com/nix-rust/nix/pull/722))
- Added `nix::unistd:fexecve`.
  ([#727](https://github.com/nix-rust/nix/pull/727))
- Expose `uname()` on all platforms.
  ([#739](https://github.com/nix-rust/nix/pull/739))
- Expose `signalfd` module on Android as well.
  ([#739](https://github.com/nix-rust/nix/pull/739))
- Added `nix::sys::ptrace::detach`.
  ([#749](https://github.com/nix-rust/nix/pull/749))
- Added timestamp socket control message variant:
  `nix::sys::socket::ControlMessage::ScmTimestamp`
  ([#663](https://github.com/nix-rust/nix/pull/663))
- Added socket option variant that enables the timestamp socket
  control message: `nix::sys::socket::sockopt::ReceiveTimestamp`
  ([#663](https://github.com/nix-rust/nix/pull/663))
- Added more accessor methods for `AioCb`
  ([#773](https://github.com/nix-rust/nix/pull/773))
- Add `nix::sys::fallocate`
  ([#768](https:://github.com/nix-rust/nix/pull/768))
- Added `nix::unistd::mkfifo`.
  ([#602](https://github.com/nix-rust/nix/pull/774))
- Added `ptrace::Options::PTRACE_O_EXITKILL` on Linux and Android.
  ([#771](https://github.com/nix-rust/nix/pull/771))
- Added `nix::sys::uio::{process_vm_readv, process_vm_writev}` on Linux
  ([#568](https://github.com/nix-rust/nix/pull/568))
- Added `nix::unistd::{getgroups, setgroups, getgrouplist, initgroups}`. ([#733](https://github.com/nix-rust/nix/pull/733))
- Added `nix::sys::socket::UnixAddr::as_abstract` on Linux and Android.
  ([#785](https://github.com/nix-rust/nix/pull/785))
- Added `nix::unistd::execveat` on Linux and Android.
  ([#800](https://github.com/nix-rust/nix/pull/800))
- Added the `from_raw()` method to `WaitStatus` for converting raw status values
  to `WaitStatus` independent of syscalls.
  ([#741](https://github.com/nix-rust/nix/pull/741))
- Added more standard trait implementations for various types.
  ([#814](https://github.com/nix-rust/nix/pull/814))
- Added `sigprocmask` to the signal module.
  ([#826](https://github.com/nix-rust/nix/pull/826))
- Added `nix::sys::socket::LinkAddr` on Linux and all bsdlike system.
  ([#813](https://github.com/nix-rust/nix/pull/813))
- Add socket options for `IP_TRANSPARENT` / `BIND_ANY`.
  ([#835](https://github.com/nix-rust/nix/pull/835))

### Changed
- Exposed the `mqueue` module for all supported operating systems.
  ([#834](https://github.com/nix-rust/nix/pull/834))
- Use native `pipe2` on all BSD targets.  Users should notice no difference.
  ([#777](https://github.com/nix-rust/nix/pull/777))
- Renamed existing `ptrace` wrappers to encourage namespacing ([#692](https://github.com/nix-rust/nix/pull/692))
- Marked `sys::ptrace::ptrace` as `unsafe`.
- Changed function signature of `socket()` and `socketpair()`. The `protocol` argument
  has changed type from `c_int` to `SockProtocol`.
  It accepts a `None` value for default protocol that was specified with zero using `c_int`.
  ([#647](https://github.com/nix-rust/nix/pull/647))
- Made `select` easier to use, adding the ability to automatically calculate the `nfds` parameter using the new
  `FdSet::highest` ([#701](https://github.com/nix-rust/nix/pull/701))
- Exposed `unistd::setresuid` and `unistd::setresgid` on FreeBSD and OpenBSD
  ([#721](https://github.com/nix-rust/nix/pull/721))
- Refactored the `statvfs` module removing extraneous API functions and the
  `statvfs::vfs` module. Additionally  `(f)statvfs()` now return the struct
  directly. And the returned `Statvfs` struct now exposes its data through
  accessor methods. ([#729](https://github.com/nix-rust/nix/pull/729))
- The `addr` argument to `madvise` and `msync` is now `*mut` to better match the
  libc API. ([#731](https://github.com/nix-rust/nix/pull/731))
- `shm_open` and `shm_unlink` are no longer exposed on Android targets, where
  they are not officially supported. ([#731](https://github.com/nix-rust/nix/pull/731))
- `MapFlags`, `MmapAdvise`, and `MsFlags` expose some more variants and only
  officially-supported variants are provided for each target.
  ([#731](https://github.com/nix-rust/nix/pull/731))
- Marked `pty::ptsname` function as `unsafe`
  ([#744](https://github.com/nix-rust/nix/pull/744))
- Moved constants ptrace request, event and options to enums and updated ptrace functions and argument types accordingly.
  ([#749](https://github.com/nix-rust/nix/pull/749))
- `AioCb::Drop` will now panic if the `AioCb` is still in-progress ([#715](https://github.com/nix-rust/nix/pull/715))
- Restricted `nix::sys::socket::UnixAddr::new_abstract` to Linux and Android only.
  ([#785](https://github.com/nix-rust/nix/pull/785))
- The `ucred` struct has been removed in favor of a `UserCredentials` struct that
  contains only getters for its fields.
  ([#814](https://github.com/nix-rust/nix/pull/814))
- Both `ip_mreq` and `ipv6_mreq` have been replaced with `IpMembershipRequest` and
  `Ipv6MembershipRequest`.
  ([#814](https://github.com/nix-rust/nix/pull/814))
- Removed return type from `pause`.
  ([#829](https://github.com/nix-rust/nix/pull/829))
- Changed the termios APIs to allow for using a `u32` instead of the `BaudRate`
  enum on BSD platforms to support arbitrary baud rates. See the module docs for
  `nix::sys::termios` for more details.
  ([#843](https://github.com/nix-rust/nix/pull/843))

### Fixed
- Fix compilation and tests for OpenBSD targets
  ([#688](https://github.com/nix-rust/nix/pull/688))
- Fixed error handling in `AioCb::fsync`, `AioCb::read`, and `AioCb::write`.
  It is no longer an error to drop an `AioCb` that failed to enqueue in the OS.
  ([#715](https://github.com/nix-rust/nix/pull/715))
- Fix potential memory corruption on non-Linux platforms when using
  `sendmsg`/`recvmsg`, caused by mismatched `msghdr` definition.
  ([#648](https://github.com/nix-rust/nix/pull/648))

### Removed
- `AioCb::from_boxed_slice` has been removed.  It was never actually safe.  Use
  `from_bytes` or `from_bytes_mut` instead.
  ([#820](https://github.com/nix-rust/nix/pull/820))
- The syscall module has been removed. This only exposed enough functionality for
  `memfd_create()` and `pivot_root()`, which are still exposed as separate functions.
  ([#747](https://github.com/nix-rust/nix/pull/747))
- The `Errno` variants are no longer reexported from the `errno` module. `Errno` itself is no longer reexported from the
  crate root and instead must be accessed using the `errno` module. ([#696](https://github.com/nix-rust/nix/pull/696))
- Removed `MS_VERBOSE`, `MS_NOSEC`, and `MS_BORN` from `MsFlags`. These
  are internal kernel flags and should never have been exposed.
  ([#814](https://github.com/nix-rust/nix/pull/814))


## [0.9.0] 2017-07-23

### Added
- Added `sysconf`, `pathconf`, and `fpathconf`
  ([#630](https://github.com/nix-rust/nix/pull/630)
- Added `sys::signal::SigAction::{ flags, mask, handler}`
  ([#611](https://github.com/nix-rust/nix/pull/609)
- Added `nix::sys::pthread::pthread_self`
  ([#591](https://github.com/nix-rust/nix/pull/591)
- Added `AioCb::from_boxed_slice`
  ([#582](https://github.com/nix-rust/nix/pull/582)
- Added `nix::unistd::{openat, fstatat, readlink, readlinkat}`
  ([#551](https://github.com/nix-rust/nix/pull/551))
- Added `nix::pty::{grantpt, posix_openpt, ptsname/ptsname_r, unlockpt}`
  ([#556](https://github.com/nix-rust/nix/pull/556)
- Added `nix::ptr::openpty`
  ([#456](https://github.com/nix-rust/nix/pull/456))
- Added `nix::ptrace::{ptrace_get_data, ptrace_getsiginfo, ptrace_setsiginfo
  and nix::Error::UnsupportedOperation}`
  ([#614](https://github.com/nix-rust/nix/pull/614))
- Added `cfmakeraw`, `cfsetspeed`, and `tcgetsid`. ([#527](https://github.com/nix-rust/nix/pull/527))
- Added "bad none", "bad write_ptr", "bad write_int", and "bad readwrite" variants to the `ioctl!`
  macro. ([#670](https://github.com/nix-rust/nix/pull/670))
- On Linux and Android, added support for receiving `PTRACE_O_TRACESYSGOOD`
  events from `wait` and `waitpid` using `WaitStatus::PtraceSyscall`
  ([#566](https://github.com/nix-rust/nix/pull/566)).

### Changed
- The `ioctl!` macro and its variants now allow the generated functions to have
  doccomments. ([#661](https://github.com/nix-rust/nix/pull/661))
- Changed `ioctl!(write ...)` into `ioctl!(write_ptr ...)` and `ioctl!(write_int ..)` variants
  to more clearly separate those use cases. ([#670](https://github.com/nix-rust/nix/pull/670))
- Marked `sys::mman::{ mmap, munmap, madvise, munlock, msync }` as unsafe.
  ([#559](https://github.com/nix-rust/nix/pull/559))
- Minimum supported Rust version is now 1.13.
- Removed `revents` argument from `PollFd::new()` as it's an output argument and
  will be overwritten regardless of value.
  ([#542](https://github.com/nix-rust/nix/pull/542))
- Changed type signature of `sys::select::FdSet::contains` to make `self`
  immutable ([#564](https://github.com/nix-rust/nix/pull/564))
- Introduced wrapper types for `gid_t`, `pid_t`, and `uid_t` as `Gid`, `Pid`, and `Uid`
  respectively. Various functions have been changed to use these new types as
  arguments. ([#629](https://github.com/nix-rust/nix/pull/629))
- Fixed compilation on all Android and iOS targets ([#527](https://github.com/nix-rust/nix/pull/527))
  and promoted them to Tier 2 support.
- `nix::sys::statfs::{statfs,fstatfs}` uses statfs definition from `libc::statfs` instead of own linux specific type `nix::sys::Statfs`.
  Also file system type constants like `nix::sys::statfs::ADFS_SUPER_MAGIC` were removed in favor of the libc equivalent.
  ([#561](https://github.com/nix-rust/nix/pull/561))
- Revised the termios API including additional tests and documentation and exposed it on iOS. ([#527](https://github.com/nix-rust/nix/pull/527))
- `eventfd`, `signalfd`, and `pwritev`/`preadv` functionality is now included by default for all
  supported platforms. ([#681](https://github.com/nix-rust/nix/pull/561))
- The `ioctl!` macro's plain variants has been replaced with "bad read" to be consistent with
  other variants. The generated functions also have more strict types for their arguments. The
  "*_buf" variants also now calculate total array size and take slice references for improved type
  safety. The documentation has also been dramatically improved.
  ([#670](https://github.com/nix-rust/nix/pull/670))

### Removed
- Removed `io::Error` from `nix::Error` and the conversion from `nix::Error` to `Errno`
  ([#614](https://github.com/nix-rust/nix/pull/614))
- All feature flags have been removed in favor of conditional compilation on supported platforms.
  `execvpe` is no longer supported, but this was already broken and will be added back in the next
  release. ([#681](https://github.com/nix-rust/nix/pull/561))
- Removed `ioc_*` functions and many helper constants and macros within the `ioctl` module. These
  should always have been private and only the `ioctl!` should be used in public code.
  ([#670](https://github.com/nix-rust/nix/pull/670))

### Fixed
- Fixed multiple issues compiling under different archetectures and OSes.
  Now compiles on Linux/MIPS ([#538](https://github.com/nix-rust/nix/pull/538)),
  `Linux/PPC` ([#553](https://github.com/nix-rust/nix/pull/553)),
  `MacOS/x86_64,i686` ([#553](https://github.com/nix-rust/nix/pull/553)),
  `NetBSD/x64_64` ([#538](https://github.com/nix-rust/nix/pull/538)),
  `FreeBSD/x86_64,i686` ([#536](https://github.com/nix-rust/nix/pull/536)), and
  `Android` ([#631](https://github.com/nix-rust/nix/pull/631)).
- `bind` and `errno_location` now work correctly on `Android`
  ([#631](https://github.com/nix-rust/nix/pull/631))
- Added `nix::ptrace` on all Linux-kernel-based platforms
  [#624](https://github.com/nix-rust/nix/pull/624). Previously it was
  only available on x86, x86-64, and ARM, and also not on Android.
- Fixed `sys::socket::sendmsg` with zero entry `cmsgs` parameter.
  ([#623](https://github.com/nix-rust/nix/pull/623))
- Multiple constants related to the termios API have now been properly defined for
  all supported platforms. ([#527](https://github.com/nix-rust/nix/pull/527))
- `ioctl!` macro now supports working with non-int datatypes and properly supports all platforms.
  ([#670](https://github.com/nix-rust/nix/pull/670))

## [0.8.1] 2017-04-16

### Fixed
- Fixed build on FreeBSD. (Cherry-picked
  [a859ee3c](https://github.com/nix-rust/nix/commit/a859ee3c9396dfdb118fcc2c8ecc697e2d303467))

## [0.8.0] 2017-03-02

### Added
- Added `::nix::sys::termios::BaudRate` enum to provide portable baudrate
  values. ([#518](https://github.com/nix-rust/nix/pull/518))
- Added a new `WaitStatus::PtraceEvent` to support ptrace events on Linux
  and Android ([#438](https://github.com/nix-rust/nix/pull/438))
- Added support for POSIX AIO
  ([#483](https://github.com/nix-rust/nix/pull/483))
  ([#506](https://github.com/nix-rust/nix/pull/506))
- Added support for XNU system control sockets
  ([#478](https://github.com/nix-rust/nix/pull/478))
- Added support for `ioctl` calls on BSD platforms
  ([#478](https://github.com/nix-rust/nix/pull/478))
- Added struct `TimeSpec`
  ([#475](https://github.com/nix-rust/nix/pull/475))
  ([#483](https://github.com/nix-rust/nix/pull/483))
- Added complete definitions for all kqueue-related constants on all supported
  OSes
  ([#415](https://github.com/nix-rust/nix/pull/415))
- Added function `epoll_create1` and bitflags `EpollCreateFlags` in
  `::nix::sys::epoll` in order to support `::libc::epoll_create1`.
  ([#410](https://github.com/nix-rust/nix/pull/410))
- Added `setresuid` and `setresgid` for Linux in `::nix::unistd`
  ([#448](https://github.com/nix-rust/nix/pull/448))
- Added `getpgid` in `::nix::unistd`
  ([#433](https://github.com/nix-rust/nix/pull/433))
- Added `tcgetpgrp` and `tcsetpgrp` in `::nix::unistd`
  ([#451](https://github.com/nix-rust/nix/pull/451))
- Added `CLONE_NEWCGROUP` in `::nix::sched`
  ([#457](https://github.com/nix-rust/nix/pull/457))
- Added `getpgrp` in `::nix::unistd`
  ([#491](https://github.com/nix-rust/nix/pull/491))
- Added `fchdir` in `::nix::unistd`
  ([#497](https://github.com/nix-rust/nix/pull/497))
- Added `major` and `minor` in `::nix::sys::stat` for decomposing `dev_t`
  ([#508](https://github.com/nix-rust/nix/pull/508))
- Fixed the style of many bitflags and use `libc` in more places.
  ([#503](https://github.com/nix-rust/nix/pull/503))
- Added `ppoll` in `::nix::poll`
  ([#520](https://github.com/nix-rust/nix/pull/520))
- Added support for getting and setting pipe size with fcntl(2) on Linux
  ([#540](https://github.com/nix-rust/nix/pull/540))

### Changed
- `::nix::sys::termios::{cfgetispeed, cfsetispeed, cfgetospeed, cfsetospeed}`
  switched  to use `BaudRate` enum from `speed_t`.
  ([#518](https://github.com/nix-rust/nix/pull/518))
- `epoll_ctl` now could accept None as argument `event`
  when op is `EpollOp::EpollCtlDel`.
  ([#480](https://github.com/nix-rust/nix/pull/480))
- Removed the `bad` keyword from the `ioctl!` macro
  ([#478](https://github.com/nix-rust/nix/pull/478))
- Changed `TimeVal` into an opaque Newtype
  ([#475](https://github.com/nix-rust/nix/pull/475))
- `kill`'s signature, defined in `::nix::sys::signal`, changed, so that the
  signal parameter has type `T: Into<Option<Signal>>`. `None` as an argument
  for that parameter will result in a 0 passed to libc's `kill`, while a
  `Some`-argument will result in the previous behavior for the contained
  `Signal`.
  ([#445](https://github.com/nix-rust/nix/pull/445))
- The minimum supported version of rustc is now 1.7.0.
  ([#444](https://github.com/nix-rust/nix/pull/444))
- Changed `KEvent` to an opaque structure that may only be modified by its
  constructor and the `ev_set` method.
  ([#415](https://github.com/nix-rust/nix/pull/415))
  ([#442](https://github.com/nix-rust/nix/pull/442))
  ([#463](https://github.com/nix-rust/nix/pull/463))
- `pipe2` now calls `libc::pipe2` where available. Previously it was emulated
  using `pipe`, which meant that setting `O_CLOEXEC` was not atomic.
  ([#427](https://github.com/nix-rust/nix/pull/427))
- Renamed `EpollEventKind` to `EpollFlags` in `::nix::sys::epoll` in order for
  it to conform with our conventions.
  ([#410](https://github.com/nix-rust/nix/pull/410))
- `EpollEvent` in `::nix::sys::epoll` is now an opaque proxy for
  `::libc::epoll_event`. The formerly public field `events` is now be read-only
  accessible with the new method `events()` of `EpollEvent`. Instances of
  `EpollEvent` can be constructed using the new method `new()` of EpollEvent.
  ([#410](https://github.com/nix-rust/nix/pull/410))
- `SigFlags` in `::nix::sys::signal` has be renamed to `SigmaskHow` and its type
  has changed from `bitflags` to `enum` in order to conform to our conventions.
  ([#460](https://github.com/nix-rust/nix/pull/460))
- `sethostname` now takes a `&str` instead of a `&[u8]` as this provides an API
  that makes more sense in normal, correct usage of the API.
- `gethostname` previously did not expose the actual length of the hostname
  written from the underlying system call at all.  This has been updated to
  return a `&CStr` within the provided buffer that is always properly
  NUL-terminated (this is not guaranteed by the call with all platforms/libc
  implementations).
- Exposed all fcntl(2) operations at the module level, so they can be
  imported direclty instead of via `FcntlArg` enum.
  ([#541](https://github.com/nix-rust/nix/pull/541))

### Fixed
- Fixed multiple issues with Unix domain sockets on non-Linux OSes
  ([#474](https://github.com/nix-rust/nix/pull/415))
- Fixed using kqueue with `EVFILT_USER` on FreeBSD
  ([#415](https://github.com/nix-rust/nix/pull/415))
- Fixed the build on FreeBSD, and fixed the getsockopt, sendmsg, and recvmsg
  functions on that same OS.
  ([#397](https://github.com/nix-rust/nix/pull/397))
- Fixed an off-by-one bug in `UnixAddr::new_abstract` in `::nix::sys::socket`.
  ([#429](https://github.com/nix-rust/nix/pull/429))
- Fixed clone passing a potentially unaligned stack.
  ([#490](https://github.com/nix-rust/nix/pull/490))
- Fixed mkdev not creating a `dev_t` the same way as libc.
  ([#508](https://github.com/nix-rust/nix/pull/508))

## [0.7.0] 2016-09-09

### Added
- Added `lseek` and `lseek64` in `::nix::unistd`
  ([#377](https://github.com/nix-rust/nix/pull/377))
- Added `mkdir` and `getcwd` in `::nix::unistd`
  ([#416](https://github.com/nix-rust/nix/pull/416))
- Added accessors `sigmask_mut` and `sigmask` to `UContext` in
  `::nix::ucontext`.
  ([#370](https://github.com/nix-rust/nix/pull/370))
- Added `WUNTRACED` to `WaitPidFlag` in `::nix::sys::wait` for non-_linux_
  targets.
  ([#379](https://github.com/nix-rust/nix/pull/379))
- Added new module `::nix::sys::reboot` with enumeration `RebootMode` and
  functions `reboot` and `set_cad_enabled`. Currently for _linux_ only.
  ([#386](https://github.com/nix-rust/nix/pull/386))
- `FdSet` in `::nix::sys::select` now also implements `Clone`.
  ([#405](https://github.com/nix-rust/nix/pull/405))
- Added `F_FULLFSYNC` to `FcntlArg` in `::nix::fcntl` for _apple_ targets.
  ([#407](https://github.com/nix-rust/nix/pull/407))
- Added `CpuSet::unset` in `::nix::sched`.
  ([#402](https://github.com/nix-rust/nix/pull/402))
- Added constructor method `new()` to `PollFd` in `::nix::poll`, in order to
  allow creation of objects, after removing public access to members.
  ([#399](https://github.com/nix-rust/nix/pull/399))
- Added method `revents()` to `PollFd` in `::nix::poll`, in order to provide
  read access to formerly public member `revents`.
  ([#399](https://github.com/nix-rust/nix/pull/399))
- Added `MSG_CMSG_CLOEXEC` to `MsgFlags` in `::nix::sys::socket` for _linux_ only.
  ([#422](https://github.com/nix-rust/nix/pull/422))

### Changed
- Replaced the reexported integer constants for signals by the enumeration
  `Signal` in `::nix::sys::signal`.
  ([#362](https://github.com/nix-rust/nix/pull/362))
- Renamed `EventFdFlag` to `EfdFlags` in `::nix::sys::eventfd`.
  ([#383](https://github.com/nix-rust/nix/pull/383))
- Changed the result types of `CpuSet::is_set` and `CpuSet::set` in
  `::nix::sched` to `Result<bool>` and `Result<()>`, respectively. They now
  return `EINVAL`, if an invalid argument for the `field` parameter is passed.
  ([#402](https://github.com/nix-rust/nix/pull/402))
- `MqAttr` in `::nix::mqueue` is now an opaque proxy for `::libc::mq_attr`,
  which has the same structure as the old `MqAttr`. The field `mq_flags` of
  `::libc::mq_attr` is readable using the new method `flags()` of `MqAttr`.
  `MqAttr` also no longer implements `Debug`.
  ([#392](https://github.com/nix-rust/nix/pull/392))
- The parameter `msq_prio` of `mq_receive` with type `u32` in `::nix::mqueue`
  was replaced by a parameter named `msg_prio` with type `&mut u32`, so that
  the message priority can be obtained by the caller.
  ([#392](https://github.com/nix-rust/nix/pull/392))
- The type alias `MQd` in `::nix::queue` was replaced by the type alias
  `libc::mqd_t`, both of which are aliases for the same type.
  ([#392](https://github.com/nix-rust/nix/pull/392))

### Removed
- Type alias `SigNum` from `::nix::sys::signal`.
  ([#362](https://github.com/nix-rust/nix/pull/362))
- Type alias `CpuMask` from `::nix::shed`.
  ([#402](https://github.com/nix-rust/nix/pull/402))
- Removed public fields from `PollFd` in `::nix::poll`. (See also added method
  `revents()`.
  ([#399](https://github.com/nix-rust/nix/pull/399))

### Fixed
- Fixed the build problem for NetBSD (Note, that we currently do not support
  it, so it might already be broken again).
  ([#389](https://github.com/nix-rust/nix/pull/389))
- Fixed the build on FreeBSD, and fixed the getsockopt, sendmsg, and recvmsg
  functions on that same OS.
  ([#397](https://github.com/nix-rust/nix/pull/397))

## [0.6.0] 2016-06-10

### Added
- Added `gettid` in `::nix::unistd` for _linux_ and _android_.
  ([#293](https://github.com/nix-rust/nix/pull/293))
- Some _mips_ support in `::nix::sched` and `::nix::sys::syscall`.
  ([#301](https://github.com/nix-rust/nix/pull/301))
- Added `SIGNALFD_SIGINFO_SIZE` in `::nix::sys::signalfd`.
  ([#309](https://github.com/nix-rust/nix/pull/309))
- Added new module `::nix::ucontext` with struct `UContext`. Currently for
  _linux_ only.
  ([#311](https://github.com/nix-rust/nix/pull/311))
- Added `EPOLLEXCLUSIVE` to `EpollEventKind` in `::nix::sys::epoll`.
  ([#330](https://github.com/nix-rust/nix/pull/330))
- Added `pause` to `::nix::unistd`.
  ([#336](https://github.com/nix-rust/nix/pull/336))
- Added `sleep` to `::nix::unistd`.
  ([#351](https://github.com/nix-rust/nix/pull/351))
- Added `S_IFDIR`, `S_IFLNK`, `S_IFMT` to `SFlag` in `::nix::sys::stat`.
  ([#359](https://github.com/nix-rust/nix/pull/359))
- Added `clear` and `extend` functions to `SigSet`'s implementation in
  `::nix::sys::signal`.
  ([#347](https://github.com/nix-rust/nix/pull/347))
- `sockaddr_storage_to_addr` in `::nix::sys::socket` now supports `sockaddr_nl`
  on _linux_ and _android_.
  ([#366](https://github.com/nix-rust/nix/pull/366))
- Added support for `SO_ORIGINAL_DST` in `::nix::sys::socket` on _linux_.
  ([#367](https://github.com/nix-rust/nix/pull/367))
- Added `SIGINFO` in `::nix::sys::signal` for the _macos_ target as well as
  `SIGPWR` and `SIGSTKFLT` in `::nix::sys::signal` for non-_macos_ targets.
  ([#361](https://github.com/nix-rust/nix/pull/361))

### Changed
- Changed the structure `IoVec` in `::nix::sys::uio`.
  ([#304](https://github.com/nix-rust/nix/pull/304))
- Replaced `CREATE_NEW_FD` by `SIGNALFD_NEW` in `::nix::sys::signalfd`.
  ([#309](https://github.com/nix-rust/nix/pull/309))
- Renamed `SaFlag` to `SaFlags` and `SigFlag` to `SigFlags` in
  `::nix::sys::signal`.
  ([#314](https://github.com/nix-rust/nix/pull/314))
- Renamed `Fork` to `ForkResult` and changed its fields in `::nix::unistd`.
  ([#332](https://github.com/nix-rust/nix/pull/332))
- Added the `signal` parameter to `clone`'s signature in `::nix::sched`.
  ([#344](https://github.com/nix-rust/nix/pull/344))
- `execv`, `execve`, and `execvp` now return `Result<Void>` instead of
  `Result<()>` in `::nix::unistd`.
  ([#357](https://github.com/nix-rust/nix/pull/357))

### Fixed
- Improved the conversion from `std::net::SocketAddr` to `InetAddr` in
  `::nix::sys::socket::addr`.
  ([#335](https://github.com/nix-rust/nix/pull/335))

## [0.5.0] 2016-03-01
