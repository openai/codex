# Changelog

## [0.2.175](https://github.com/rust-lang/libc/compare/0.2.174...0.2.175) - 2025-08-10

### Added

- AIX: Add `getpeereid` ([#4524](https://github.com/rust-lang/libc/pull/4524))
- AIX: Add `struct ld_info` and friends ([#4578](https://github.com/rust-lang/libc/pull/4578))
- AIX: Retore `struct winsize` ([#4577](https://github.com/rust-lang/libc/pull/4577))
- Android: Add UDP socket option constants ([#4619](https://github.com/rust-lang/libc/pull/4619))
- Android: Add `CLONE_CLEAR_SIGHAND` and `CLONE_INTO_CGROUP` ([#4502](https://github.com/rust-lang/libc/pull/4502))
- Android: Add more `prctl` constants ([#4531](https://github.com/rust-lang/libc/pull/4531))
- FreeBSD Add further TCP stack-related constants ([#4196](https://github.com/rust-lang/libc/pull/4196))
- FreeBSD x86-64: Add `mcontext_t.mc_tlsbase ` ([#4503](https://github.com/rust-lang/libc/pull/4503))
- FreeBSD15: Add `kinfo_proc.ki_uerrmsg` ([#4552](https://github.com/rust-lang/libc/pull/4552))
- FreeBSD: Add `in_conninfo` ([#4482](https://github.com/rust-lang/libc/pull/4482))
- FreeBSD: Add `xinpgen` and related types ([#4482](https://github.com/rust-lang/libc/pull/4482))
- FreeBSD: Add `xktls_session` ([#4482](https://github.com/rust-lang/libc/pull/4482))
- Haiku: Add functionality from `libbsd` ([#4221](https://github.com/rust-lang/libc/pull/4221))
- Linux: Add `SECBIT_*` ([#4480](https://github.com/rust-lang/libc/pull/4480))
- NetBSD, OpenBSD: Export `ioctl` request generator macros ([#4460](https://github.com/rust-lang/libc/pull/4460))
- NetBSD: Add `ptsname_r` ([#4608](https://github.com/rust-lang/libc/pull/4608))
- RISCV32: Add time-related syscalls ([#4612](https://github.com/rust-lang/libc/pull/4612))
- Solarish: Add `strftime*` ([#4453](https://github.com/rust-lang/libc/pull/4453))
- linux: Add `EXEC_RESTRICT_*` and `EXEC_DENY_*` ([#4545](https://github.com/rust-lang/libc/pull/4545))

### Changed

- AIX: Add `const` to signatures to be consistent with other platforms ([#4563](https://github.com/rust-lang/libc/pull/4563))

### Fixed

- AIX: Fix the type of `struct statvfs.f_fsid` ([#4576](https://github.com/rust-lang/libc/pull/4576))
- AIX: Fix the type of constants for the `ioctl` `request` argument ([#4582](https://github.com/rust-lang/libc/pull/4582))
- AIX: Fix the types of `stat{,64}.st_*tim` ([#4597](https://github.com/rust-lang/libc/pull/4597))
- AIX: Use unique `errno` values ([#4507](https://github.com/rust-lang/libc/pull/4507))
- Build: Fix an incorrect `target_os` -> `target_arch` check ([#4550](https://github.com/rust-lang/libc/pull/4550))
- FreeBSD: Fix the type of `xktls_session_onedir.ifnet` ([#4552](https://github.com/rust-lang/libc/pull/4552))
- Mips64 musl: Fix the type of `nlink_t` ([#4509](https://github.com/rust-lang/libc/pull/4509))
- Mips64 musl: Use a special MIPS definition of `stack_t` ([#4528](https://github.com/rust-lang/libc/pull/4528))
- Mips64: Fix `SI_TIMER`, `SI_MESGQ` and `SI_ASYNCIO` definitions ([#4529](https://github.com/rust-lang/libc/pull/4529))
- Musl Mips64: Swap the order of `si_errno` and `si_code` in `siginfo_t` ([#4530](https://github.com/rust-lang/libc/pull/4530))
- Musl Mips64: Use a special MIPS definition of `statfs` ([#4527](https://github.com/rust-lang/libc/pull/4527))
- Musl: Fix the definition of `fanotify_event_metadata` ([#4510](https://github.com/rust-lang/libc/pull/4510))
- NetBSD: Correct `enum fae_action` to be `#[repr(C)]` ([#60a8cfd5](https://github.com/rust-lang/libc/commit/60a8cfd564f83164d45b9533ff7a0d7371878f2a))
- PSP: Correct `char` -> `c_char` ([eaab4fc3](https://github.com/rust-lang/libc/commit/eaab4fc3f05dc646a953d4fd5ba46dfa1f8bd6f6))
- PowerPC musl: Fix `termios` definitions ([#4518](https://github.com/rust-lang/libc/pull/4518))
- PowerPC musl: Fix the definition of `EDEADLK` ([#4517](https://github.com/rust-lang/libc/pull/4517))
- PowerPC musl: Fix the definition of `NCCS` ([#4513](https://github.com/rust-lang/libc/pull/4513))
- PowerPC musl: Fix the definitions of `MAP_LOCKED` and `MAP_NORESERVE` ([#4516](https://github.com/rust-lang/libc/pull/4516))
- PowerPC64 musl: Fix the definition of `shmid_ds` ([#4519](https://github.com/rust-lang/libc/pull/4519))

### Deprecated

- Linux: `MAP_32BIT` is only defined on x86 on non-x86 architectures ([#4511](https://github.com/rust-lang/libc/pull/4511))

### Removed

- AIX: Remove duplicate constant definitions `FIND` and `ENTER` ([#4588](https://github.com/rust-lang/libc/pull/4588))
- s390x musl: Remove `O_FSYNC` ([#4515](https://github.com/rust-lang/libc/pull/4515))
- s390x musl: Remove `RTLD_DEEPBIND` ([#4515](https://github.com/rust-lang/libc/pull/4515))


## [0.2.174](https://github.com/rust-lang/libc/compare/0.2.173...0.2.174) - 2025-06-17

### Added

- Linux: Make `pidfd_info` fields pub ([#4487](https://github.com/rust-lang/libc/pull/4487))

### Fixed

- Gnu x32: Add missing `timespec.tv_nsec` ([#4497](https://github.com/rust-lang/libc/pull/4497))
- NuttX: Use `nlink_t` type for `st_nlink` in `struct stat` definition ([#4483](https://github.com/rust-lang/libc/pull/4483))

### Other

- Allow new `unpredictable_function_pointer_comparisons` lints ([#4489](https://github.com/rust-lang/libc/pull/4489))
- OpenBSD: Fix some clippy warnings to use `pointer::cast`. ([#4490](https://github.com/rust-lang/libc/pull/4490))
- Remove unessecary semicolons from definitions of `CMSG_NXTHDR`. ([#4492](https://github.com/rust-lang/libc/pull/4492))


## [0.2.173](https://github.com/rust-lang/libc/compare/0.2.172...0.2.173) - 2025-06-09

### Added

- AIX: Add an AIX triple to Cargo.toml for doc ([#4475](https://github.com/rust-lang/libc/pull/4475))
- FreeBSD: Add the `SO_SPLICE` socket option support for FreeBSD >= 14.2 ([#4451](https://github.com/rust-lang/libc/pull/4451))
- Linux GNU: Prepare for supporting `_TIME_BITS=64` ([#4433](https://github.com/rust-lang/libc/pull/4433))
- Linux: Add constant PACKET_IGNORE_OUTGOING ([#4319](https://github.com/rust-lang/libc/pull/4319))
- Linux: Add constants and types for `nsfs` ioctls ([#4436](https://github.com/rust-lang/libc/pull/4436))
- Linux: Add constants for Memory-Deny-Write-Execute `prctls` ([#4400](https://github.com/rust-lang/libc/pull/4400))
- Linux: Add constants from `linux/cn_proc.h` and `linux/connector.h` ([#4434](https://github.com/rust-lang/libc/pull/4434))
- Linux: Add new flags for `pwritev2` and `preadv2` ([#4452](https://github.com/rust-lang/libc/pull/4452))
- Linux: Add pid_type enum values ([#4403](https://github.com/rust-lang/libc/pull/4403))
- Linux: Update pidfd constants and types (Linux 6.9-6.15) ([#4402](https://github.com/rust-lang/libc/pull/4402))
- Loongarch64 musl: Define the `MADV_SOFT_OFFLINE` constant ([#4448](https://github.com/rust-lang/libc/pull/4448))
- Musl: Add new fields since 1.2.0/1.2.2 to `struct tcp_info` ([#4443](https://github.com/rust-lang/libc/pull/4443))
- Musl: Prepare for supporting v1.2.3 ([#4443](https://github.com/rust-lang/libc/pull/4443))
- NuttX: Add `arc4random` and `arc4random_buf` ([#4464](https://github.com/rust-lang/libc/pull/4464))
- RISC-V Musl: Add `MADV_SOFT_OFFLINE` definition ([#4447](https://github.com/rust-lang/libc/pull/4447))
- Redox: Define SCM_RIGHTS ([#4440](https://github.com/rust-lang/libc/pull/4440))
- VxWorks: Add missing UTIME defines and TASK_RENAME_LENGTH ([#4407](https://github.com/rust-lang/libc/pull/4407))
- Windows: Add more `time.h` functions ([#4427](https://github.com/rust-lang/libc/pull/4427))

### Changed

- Redox: Update `SA_` constants. ([#4426](https://github.com/rust-lang/libc/pull/4426))
- Redox: make `CMSG_ALIGN`, `CMSG_LEN`, and `CMSG_SPACE` const functions ([#4441](https://github.com/rust-lang/libc/pull/4441))

### Fixed

- AIX: Enable libc-test and fix definitions/declarations. ([#4450](https://github.com/rust-lang/libc/pull/4450))
- Emscripten: Fix querying emcc on windows (use emcc.bat) ([#4248](https://github.com/rust-lang/libc/pull/4248))
- Hurd: Fix build from missing `fpos_t` ([#4472](https://github.com/rust-lang/libc/pull/4472))
- Loongarch64 Musl: Fix the `struct ipc_perm` bindings ([#4384](https://github.com/rust-lang/libc/pull/4384))
- Musl: Fix the `O_LARGEFILE` constant value. ([#4443](https://github.com/rust-lang/libc/pull/4443))

## [0.2.172](https://github.com/rust-lang/libc/compare/0.2.171...0.2.172) - 2025-04-14

### Added

- Android: Add `getauxval` for 32-bit targets ([#4338](https://github.com/rust-lang/libc/pull/4338))
- Android: Add `if_tun.h` ioctls ([#4379](https://github.com/rust-lang/libc/pull/4379))
- Android: Define `SO_BINDTOIFINDEX` ([#4391](https://github.com/rust-lang/libc/pull/4391))
- Cygwin: Add `posix_spawn_file_actions_add[f]chdir[_np]` ([#4387](https://github.com/rust-lang/libc/pull/4387))
- Cygwin: Add new socket options ([#4350](https://github.com/rust-lang/libc/pull/4350))
- Cygwin: Add statfs & fcntl ([#4321](https://github.com/rust-lang/libc/pull/4321))
- FreeBSD: Add `filedesc` and `fdescenttbl` ([#4327](https://github.com/rust-lang/libc/pull/4327))
- Glibc: Add unstable support for _FILE_OFFSET_BITS=64 ([#4345](https://github.com/rust-lang/libc/pull/4345))
- Hermit: Add `AF_UNSPEC` ([#4344](https://github.com/rust-lang/libc/pull/4344))
- Hermit: Add `AF_VSOCK` ([#4344](https://github.com/rust-lang/libc/pull/4344))
- Illumos, NetBSD: Add `timerfd` APIs ([#4333](https://github.com/rust-lang/libc/pull/4333))
- Linux: Add `_IO`, `_IOW`, `_IOR`, `_IOWR` to the exported API ([#4325](https://github.com/rust-lang/libc/pull/4325))
- Linux: Add `tcp_info` to uClibc bindings ([#4347](https://github.com/rust-lang/libc/pull/4347))
- Linux: Add further BPF program flags ([#4356](https://github.com/rust-lang/libc/pull/4356))
- Linux: Add missing INPUT_PROP_XXX flags from `input-event-codes.h` ([#4326](https://github.com/rust-lang/libc/pull/4326))
- Linux: Add missing TLS bindings ([#4296](https://github.com/rust-lang/libc/pull/4296))
- Linux: Add more constants from `seccomp.h` ([#4330](https://github.com/rust-lang/libc/pull/4330))
- Linux: Add more glibc `ptrace_sud_config` and related `PTRACE_*ET_SYSCALL_USER_DISPATCH_CONFIG`. ([#4386](https://github.com/rust-lang/libc/pull/4386))
- Linux: Add new netlink flags ([#4288](https://github.com/rust-lang/libc/pull/4288))
- Linux: Define ioctl codes on more architectures ([#4382](https://github.com/rust-lang/libc/pull/4382))
- Linux: Add missing `pthread_attr_setstack` ([#4349](https://github.com/rust-lang/libc/pull/4349))
- Musl: Add missing `utmpx` API ([#4332](https://github.com/rust-lang/libc/pull/4332))
- Musl: Enable `getrandom` on all platforms ([#4346](https://github.com/rust-lang/libc/pull/4346))
- NuttX: Add more signal constants ([#4353](https://github.com/rust-lang/libc/pull/4353))
- QNX: Add QNX 7.1-iosock and 8.0 to list of additional cfgs ([#4169](https://github.com/rust-lang/libc/pull/4169))
- QNX: Add support for alternative Neutrino network stack `io-sock` ([#4169](https://github.com/rust-lang/libc/pull/4169))
- Redox: Add more `sys/socket.h` and `sys/uio.h` definitions ([#4388](https://github.com/rust-lang/libc/pull/4388))
- Solaris: Temporarily define `O_DIRECT` and `SIGINFO` ([#4348](https://github.com/rust-lang/libc/pull/4348))
- Solarish: Add `secure_getenv` ([#4342](https://github.com/rust-lang/libc/pull/4342))
- VxWorks: Add missing `d_type` member to `dirent` ([#4352](https://github.com/rust-lang/libc/pull/4352))
- VxWorks: Add missing signal-related constsants ([#4352](https://github.com/rust-lang/libc/pull/4352))
- VxWorks: Add more error codes ([#4337](https://github.com/rust-lang/libc/pull/4337))

### Deprecated

- FreeBSD: Deprecate `TCP_PCAP_OUT` and `TCP_PCAP_IN` ([#4381](https://github.com/rust-lang/libc/pull/4381))

### Fixed

- Cygwin: Fix member types of `statfs` ([#4324](https://github.com/rust-lang/libc/pull/4324))
- Cygwin: Fix tests  ([#4357](https://github.com/rust-lang/libc/pull/4357))
- Hermit: Make `AF_INET = 3` ([#4344](https://github.com/rust-lang/libc/pull/4344))
- Musl: Fix the syscall table on RISC-V-32 ([#4335](https://github.com/rust-lang/libc/pull/4335))
- Musl: Fix the value of `SA_ONSTACK` on RISC-V-32 ([#4335](https://github.com/rust-lang/libc/pull/4335))
- VxWorks: Fix a typo in the `waitpid` parameter name ([#4334](https://github.com/rust-lang/libc/pull/4334))

### Removed

- Musl: Remove `O_FSYNC` on RISC-V-32 (use `O_SYNC` instead) ([#4335](https://github.com/rust-lang/libc/pull/4335))
- Musl: Remove `RTLD_DEEPBIND` on RISC-V-32 ([#4335](https://github.com/rust-lang/libc/pull/4335))

### Other

- CI: Add matrix env variables to the environment ([#4345](https://github.com/rust-lang/libc/pull/4345))
- CI: Always deny warnings ([#4363](https://github.com/rust-lang/libc/pull/4363))
- CI: Always upload successfully created artifacts ([#4345](https://github.com/rust-lang/libc/pull/4345))
- CI: Install musl from source for loongarch64 ([#4320](https://github.com/rust-lang/libc/pull/4320))
- CI: Revert "Also skip `MFD_EXEC` and `MFD_NOEXEC_SEAL` on sparc64" ([#]())
- CI: Use `$PWD` instead of `$(pwd)` in run-docker ([#4345](https://github.com/rust-lang/libc/pull/4345))
- Solarish: Restrict `openpty` and `forkpty` polyfills to Illumos, replace Solaris implementation with bindings ([#4329](https://github.com/rust-lang/libc/pull/4329))
- Testing: Ensure the makedev test does not emit unused errors ([#4363](https://github.com/rust-lang/libc/pull/4363))

## [0.2.171](https://github.com/rust-lang/libc/compare/0.2.170...0.2.171) - 2025-03-11

### Added

- Android: Add `if_nameindex`/`if_freenameindex` support ([#4247](https://github.com/rust-lang/libc/pull/4247))
- Apple: Add missing proc types and constants ([#4310](https://github.com/rust-lang/libc/pull/4310))
- BSD: Add `devname` ([#4285](https://github.com/rust-lang/libc/pull/4285))
- Cygwin: Add PTY and group API ([#4309](https://github.com/rust-lang/libc/pull/4309))
- Cygwin: Add support ([#4279](https://github.com/rust-lang/libc/pull/4279))
- FreeBSD: Make `spawn.h` interfaces available on all FreeBSD-like systems ([#4294](https://github.com/rust-lang/libc/pull/4294))
- Linux: Add `AF_XDP` structs for all Linux environments ([#4163](https://github.com/rust-lang/libc/pull/4163))
- Linux: Add SysV semaphore constants ([#4286](https://github.com/rust-lang/libc/pull/4286))
- Linux: Add `F_SEAL_EXEC` ([#4316](https://github.com/rust-lang/libc/pull/4316))
- Linux: Add `SO_PREFER_BUSY_POLL` and `SO_BUSY_POLL_BUDGET` ([#3917](https://github.com/rust-lang/libc/pull/3917))
- Linux: Add `devmem` structs ([#4299](https://github.com/rust-lang/libc/pull/4299))
- Linux: Add socket constants up to `SO_DEVMEM_DONTNEED` ([#4299](https://github.com/rust-lang/libc/pull/4299))
- NetBSD, OpenBSD, DragonflyBSD: Add `closefrom` ([#4290](https://github.com/rust-lang/libc/pull/4290))
- NuttX: Add `pw_passwd` field to `passwd` ([#4222](https://github.com/rust-lang/libc/pull/4222))
- Solarish: define `IP_BOUND_IF` and `IPV6_BOUND_IF` ([#4287](https://github.com/rust-lang/libc/pull/4287))
- Wali: Add bindings for `wasm32-wali-linux-musl` target ([#4244](https://github.com/rust-lang/libc/pull/4244))

### Changed

- AIX: Use `sa_sigaction` instead of a union ([#4250](https://github.com/rust-lang/libc/pull/4250))
- Make `msqid_ds.__msg_cbytes` public ([#4301](https://github.com/rust-lang/libc/pull/4301))
- Unix: Make all `major`, `minor`, `makedev` into `const fn` ([#4208](https://github.com/rust-lang/libc/pull/4208))

### Deprecated

- Linux: Deprecate obsolete packet filter interfaces ([#4267](https://github.com/rust-lang/libc/pull/4267))

### Fixed

- Cygwin: Fix strerror_r ([#4308](https://github.com/rust-lang/libc/pull/4308))
- Cygwin: Fix usage of f! ([#4308](https://github.com/rust-lang/libc/pull/4308))
- Hermit: Make `stat::st_size` signed ([#4298](https://github.com/rust-lang/libc/pull/4298))
- Linux: Correct values for `SI_TIMER`, `SI_MESGQ`, `SI_ASYNCIO` ([#4292](https://github.com/rust-lang/libc/pull/4292))
- NuttX: Update `tm_zone` and `d_name` fields to use `c_char` type ([#4222](https://github.com/rust-lang/libc/pull/4222))
- Xous: Include the prelude to define `c_int` ([#4304](https://github.com/rust-lang/libc/pull/4304))

### Other

- Add labels to FIXMEs ([#4231](https://github.com/rust-lang/libc/pull/4231), [#4232](https://github.com/rust-lang/libc/pull/4232), [#4234](https://github.com/rust-lang/libc/pull/4234), [#4235](https://github.com/rust-lang/libc/pull/4235), [#4236](https://github.com/rust-lang/libc/pull/4236))
- CI: Fix "cannot find libc" error on Sparc64 ([#4317](https://github.com/rust-lang/libc/pull/4317))
- CI: Fix "cannot find libc" error on s390x ([#4317](https://github.com/rust-lang/libc/pull/4317))
- CI: Pass `--no-self-update` to `rustup update` ([#4306](https://github.com/rust-lang/libc/pull/4306))
- CI: Remove tests for the `i586-pc-windows-msvc` target ([#4311](https://github.com/rust-lang/libc/pull/4311))
- CI: Remove the `check_cfg` job ([#4322](https://github.com/rust-lang/libc/pull/4312))
- Change the range syntax that is giving `ctest` problems ([#4311](https://github.com/rust-lang/libc/pull/4311))
- Linux: Split out the stat struct for gnu/b32/mips ([#4276](https://github.com/rust-lang/libc/pull/4276))

### Removed

- NuttX: Remove `pthread_set_name_np` ([#4251](https://github.com/rust-lang/libc/pull/4251))

## [0.2.170](https://github.com/rust-lang/libc/compare/0.2.169...0.2.170) - 2025-02-23

### Added

- Android: Declare `setdomainname` and `getdomainname` <https://github.com/rust-lang/libc/pull/4212>
- FreeBSD: Add `evdev` structures <https://github.com/rust-lang/libc/pull/3756>
- FreeBSD: Add the new `st_filerev` field to `stat32` ([#4254](https://github.com/rust-lang/libc/pull/4254))
- Linux: Add `SI_*`` and `TRAP_*`` signal codes <https://github.com/rust-lang/libc/pull/4225>
- Linux: Add experimental configuration to enable 64-bit time in kernel APIs, set by `RUST_LIBC_UNSTABLE_LINUX_TIME_BITS64`. <https://github.com/rust-lang/libc/pull/4148>
- Linux: Add recent socket timestamping flags <https://github.com/rust-lang/libc/pull/4273>
- Linux: Added new CANFD_FDF flag for the flags field of canfd_frame <https://github.com/rust-lang/libc/pull/4223>
- Musl: add CLONE_NEWTIME <https://github.com/rust-lang/libc/pull/4226>
- Solarish: add the posix_spawn family of functions <https://github.com/rust-lang/libc/pull/4259>

### Deprecated

- Linux: deprecate kernel modules syscalls <https://github.com/rust-lang/libc/pull/4228>

### Changed

- Emscripten: Assume version is at least 3.1.42 <https://github.com/rust-lang/libc/pull/4243>

### Fixed

- BSD: Correct the definition of `WEXITSTATUS` <https://github.com/rust-lang/libc/pull/4213>
- Hurd: Fix CMSG_DATA on 64bit systems ([#4240](https://github.com/rust-lang/libc/pull/424))
- NetBSD: fix `getmntinfo` ([#4265](https://github.com/rust-lang/libc/pull/4265)
- VxWorks: Fix the size of `time_t` <https://github.com/rust-lang/libc/pull/426>

### Other

- Add labels to FIXMEs <https://github.com/rust-lang/libc/pull/4230>, <https://github.com/rust-lang/libc/pull/4229>, <https://github.com/rust-lang/libc/pull/4237>
- CI: Bump FreeBSD CI to 13.4 and 14.2 <https://github.com/rust-lang/libc/pull/4260>
- Copy definitions from core::ffi and centralize them <https://github.com/rust-lang/libc/pull/4256>
- Define c_char at top-level and remove per-target c_char definitions <https://github.com/rust-lang/libc/pull/4202>
- Port style.rs to syn and add tests for the style checker <https://github.com/rust-lang/libc/pull/4220>

## [0.2.169](https://github.com/rust-lang/libc/compare/0.2.168...0.2.169) - 2024-12-18

### Added

- FreeBSD: add more socket TCP stack constants <https://github.com/rust-lang/libc/pull/4193>
- Fuchsia: add a `sockaddr_vm` definition <https://github.com/rust-lang/libc/pull/4194>

### Fixed

**Breaking**: [rust-lang/rust#132975](https://github.com/rust-lang/rust/pull/132975) corrected the signedness of `core::ffi::c_char` on various Tier 2 and Tier 3 platforms (mostly Arm and RISC-V) to match Clang. This release contains the corresponding changes to `libc`, including the following specific pull requests:

- ESP-IDF: Replace arch-conditional `c_char` with a reexport <https://github.com/rust-lang/libc/pull/4195>
- Fix `c_char` on various targets <https://github.com/rust-lang/libc/pull/4199>
- Mirror `c_char` configuration from `rust-lang/rust` <https://github.com/rust-lang/libc/pull/4198>

### Cleanup

- Do not re-export `c_void` in target-specific code <https://github.com/rust-lang/libc/pull/4200>

## [0.2.168](https://github.com/rust-lang/libc/compare/0.2.167...0.2.168) - 2024-12-09

### Added

- Linux: Add new process flags ([#4174](https://github.com/rust-lang/libc/pull/4174))
- Linux: Make `IFA_*` constants available on all Linux targets <https://github.com/rust-lang/libc/pull/4185>
- Linux: add `MAP_DROPPABLE` <https://github.com/rust-lang/libc/pull/4173>
- Solaris, Illumos: add `SIGRTMIN` and `SIGRTMAX` <https://github.com/rust-lang/libc/pull/4171>
- Unix, Linux: adding POSIX `memccpy` and `mempcpy` GNU extension <https://github.com/rust-lang/libc/pull/4186.

### Deprecated

- FreeBSD: Deprecate the CAP_UNUSED* and CAP_ALL* constants ([#4183](https://github.com/rust-lang/libc/pull/4183))

### Fixed

- Make the `Debug` implementation for unions opaque ([#4176](https://github.com/rust-lang/libc/pull/4176))

### Other

- Allow the `unpredictable_function_pointer_comparisons` lint where needed <https://github.com/rust-lang/libc/pull/4177>
- CI: Upload artifacts created by libc-test <https://github.com/rust-lang/libc/pull/4180>
- CI: Use workflow commands to group output by target <https://github.com/rust-lang/libc/pull/4179>
- CI: add caching <https://github.com/rust-lang/libc/pull/4183>

## [0.2.167](https://github.com/rust-lang/libc/compare/0.2.166...0.2.167) - 2024-11-28

### Added

- Solarish: add `st_fstype` to `stat` <https://github.com/rust-lang/libc/pull/4145>
- Trusty: Add `intptr_t` and `uintptr_t` ([#4161](https://github.com/rust-lang/libc/pull/4161))

### Fixed

- Fix the build with `rustc-dep-of-std` <https://github.com/rust-lang/libc/pull/4158>
- Wasi: Add back unsafe block for `clockid_t` static variables ([#4157](https://github.com/rust-lang/libc/pull/4157))

### Cleanup

- Create an internal prelude <https://github.com/rust-lang/libc/pull/4161>
- Fix `unused_qualifications`<https://github.com/rust-lang/libc/pull/4132>

### Other

- CI: Check various FreeBSD versions ([#4159](https://github.com/rust-lang/libc/pull/4159))
- CI: add a timeout for all jobs <https://github.com/rust-lang/libc/pull/4164>
- CI: verify MSRV for `wasm32-wasi` <https://github.com/rust-lang/libc/pull/4157>
- Migrate to the 2021 edition <https://github.com/rust-lang/libc/pull/4132>

### Removed

- Remove one unused import after the edition 2021 bump

## [0.2.166](https://github.com/rust-lang/libc/compare/0.2.165...0.2.166) - 2024-11-26

### Fixed

This release resolves two cases of unintentional breakage from the previous release:

- Revert removal of array size hacks [#4150](https://github.com/rust-lang/libc/pull/4150)
- Ensure `const extern` functions are always enabled [#4151](https://github.com/rust-lang/libc/pull/4151)

## [0.2.165](https://github.com/rust-lang/libc/compare/0.2.164...0.2.165) - 2024-11-25

### Added

- Android: add `mkostemp`, `mkostemps` <https://github.com/rust-lang/libc/pull/3601>
- Android: add a few API 30 calls <https://github.com/rust-lang/libc/pull/3604>
- Android: add missing syscall constants <https://github.com/rust-lang/libc/pull/3558>
- Apple: add `in6_ifreq` <https://github.com/rust-lang/libc/pull/3617>
- Apple: add missing `sysctl` net types <https://github.com/rust-lang/libc/pull/4022> (before release: remove `if_family_id` ([#4137](https://github.com/rust-lang/libc/pulls/4137)))
- Freebsd: add `kcmp` call support <https://github.com/rust-lang/libc/pull/3746>
- Hurd: add `MAP_32BIT` and `MAP_EXCL` <https://github.com/rust-lang/libc/pull/4127>
- Hurd: add `domainname` field to `utsname` ([#4089](https://github.com/rust-lang/libc/pulls/4089))
- Linux GNU: add `f_flags` to struct `statfs` for arm, mips, powerpc and x86 <https://github.com/rust-lang/libc/pull/3663>
- Linux GNU: add `malloc_stats` <https://github.com/rust-lang/libc/pull/3596>
- Linux: add ELF relocation-related structs <https://github.com/rust-lang/libc/pull/3583>
- Linux: add `ptp_*` structs <https://github.com/rust-lang/libc/pull/4113>
- Linux: add `ptp_clock_caps` <https://github.com/rust-lang/libc/pull/4128>
- Linux: add `ptp_pin_function` and most `PTP_` constants <https://github.com/rust-lang/libc/pull/4114>
- Linux: add missing AF_XDP structs & constants <https://github.com/rust-lang/libc/pull/3956>
- Linux: add missing netfilter consts ([#3734](https://github.com/rust-lang/libc/pulls/3734))
- Linux: add struct and constants for the `mount_setattr` syscall <https://github.com/rust-lang/libc/pull/4046>
- Linux: add wireless API <https://github.com/rust-lang/libc/pull/3441>
- Linux: expose the `len8_dlc` field of `can_frame` <https://github.com/rust-lang/libc/pull/3357>
- Musl: add `utmpx` API <https://github.com/rust-lang/libc/pull/3213>
- Musl: add missing syscall constants <https://github.com/rust-lang/libc/pull/4028>
- NetBSD: add `mcontext`-related data for RISCV64 <https://github.com/rust-lang/libc/pull/3468>
- Redox: add new `netinet` constants <https://github.com/rust-lang/libc/pull/3586>)
- Solarish: add `_POSIX_VDISABLE` ([#4103](https://github.com/rust-lang/libc/pulls/4103))
- Tests: Add a test that the `const extern fn` macro works <https://github.com/rust-lang/libc/pull/4134>
- Tests: Add test of primitive types against `std` <https://github.com/rust-lang/libc/pull/3616>
- Unix: Add `htonl`, `htons`, `ntohl`, `ntohs` <https://github.com/rust-lang/libc/pull/3669>
- Unix: add `aligned_alloc` <https://github.com/rust-lang/libc/pull/3843>
- Windows: add `aligned_realloc` <https://github.com/rust-lang/libc/pull/3592>

### Fixed

- **breaking** Hurd: fix `MAP_HASSEMAPHORE` name ([#4127](https://github.com/rust-lang/libc/pulls/4127))
- **breaking** ulibc Mips: fix `SA_*` mismatched types ([#3211](https://github.com/rust-lang/libc/pulls/3211))
- Aix: fix an enum FFI safety warning <https://github.com/rust-lang/libc/pull/3644>
- Haiku: fix some typos ([#3664](https://github.com/rust-lang/libc/pulls/3664))
- Tests: fix `Elf{32,64}_Relr`-related tests <https://github.com/rust-lang/libc/pull/3647>
- Tests: fix libc-tests for `loongarch64-linux-musl`
- Tests: fix some clippy warnings <https://github.com/rust-lang/libc/pull/3855>
- Tests: fix tests on `riscv64gc-unknown-freebsd` <https://github.com/rust-lang/libc/pull/4129>

### Deprecated

- Apple: deprecate `iconv_open` <https://github.com/rust-lang/libc/commit/25e022a22eca3634166ef472b748c297e60fcf7f>
- Apple: deprecate `mach_task_self` <https://github.com/rust-lang/libc/pull/4095>
- Apple: update `mach` deprecation notices for things that were removed in `main` <https://github.com/rust-lang/libc/pull/4097>

### Cleanup

- Adjust the `f!` macro to be more flexible <https://github.com/rust-lang/libc/pull/4107>
- Aix: remove duplicate constants <https://github.com/rust-lang/libc/pull/3643>
- CI: make scripts more uniform <https://github.com/rust-lang/libc/pull/4042>
- Drop the `libc_align` conditional <https://github.com/rust-lang/libc/commit/b5b553d0ee7de0d4781432a9a9a0a6445dd7f34f>
- Drop the `libc_cfg_target_vendor` conditional <https://github.com/rust-lang/libc/pull/4060>
- Drop the `libc_const_size_of` conditional <https://github.com/rust-lang/libc/commit/5a43dd2754366f99b3a83881b30246ce0e51833c>
- Drop the `libc_core_cvoid` conditional <https://github.com/rust-lang/libc/pull/4060>
- Drop the `libc_int128` conditional <https://github.com/rust-lang/libc/pull/4060>
- Drop the `libc_non_exhaustive` conditional <https://github.com/rust-lang/libc/pull/4060>
- Drop the `libc_packedN` conditional <https://github.com/rust-lang/libc/pull/4060>
- Drop the `libc_priv_mod_use` conditional <https://github.com/rust-lang/libc/commit/19c59376d11b015009fb9b04f233a30a1bf50a91>
- Drop the `libc_union` conditional <https://github.com/rust-lang/libc/commit/b9e4d8012f612dfe24147da3e69522763f92b6e3>
- Drop the `long_array` conditional <https://github.com/rust-lang/libc/pull/4096>
- Drop the `ptr_addr_of` conditional <https://github.com/rust-lang/libc/pull/4065>
- Drop warnings about deprecated cargo features <https://github.com/rust-lang/libc/pull/4060>
- Eliminate uses of `struct_formatter` <https://github.com/rust-lang/libc/pull/4074>
- Fix a few other array size hacks <https://github.com/rust-lang/libc/commit/d63be8b69b0736753213f5d933767866a5801ee7>
- Glibc: remove redundant definitions ([#3261](https://github.com/rust-lang/libc/pulls/3261))
- Musl: remove redundant definitions ([#3261](https://github.com/rust-lang/libc/pulls/3261))
- Musl: unify definitions of `siginfo_t` ([#3261](https://github.com/rust-lang/libc/pulls/3261))
- Musl: unify definitions of statfs and statfs64 ([#3261](https://github.com/rust-lang/libc/pulls/3261))
- Musl: unify definitions of statvfs and statvfs64 ([#3261](https://github.com/rust-lang/libc/pulls/3261))
- Musl: unify statx definitions ([#3978](https://github.com/rust-lang/libc/pulls/3978))
- Remove array size hacks for Rust < 1.47 <https://github.com/rust-lang/libc/commit/27ee6fe02ca0848b2af3cd747536264e4c7b697d>
- Remove repetitive words <https://github.com/rust-lang/libc/commit/77de375891285e18a81616f7dceda6d52732eed6>
- Use #[derive] for Copy/Clone in s! and friends <https://github.com/rust-lang/libc/pull/4038>
- Use some tricks to format macro bodies <https://github.com/rust-lang/libc/pull/4107>

### Other

- Apply formatting to macro bodies <https://github.com/rust-lang/libc/pull/4107>
- Bump libc-test to Rust 2021 Edition <https://github.com/rust-lang/libc/pull/3905>
- CI: Add a check that semver files don't contain duplicate entries <https://github.com/rust-lang/libc/pull/4087>
- CI: Add `fanotify_event_info_fid` to FAM-exempt types <https://github.com/rust-lang/libc/pull/4038>
- CI: Allow rustfmt to organize imports ([#4136](https://github.com/rust-lang/libc/pulls/4136))
- CI: Always run rustfmt <https://github.com/rust-lang/libc/pull/4120>
- CI: Change 32-bit Docker images to use EOL repos <https://github.com/rust-lang/libc/pull/4120>
- CI: Change 64-bit Docker images to ubuntu:24.10 <https://github.com/rust-lang/libc/pull/4120>
- CI: Disable the check for >1 s! invocation <https://github.com/rust-lang/libc/pull/4107>
- CI: Ensure build channels get run even if FILTER is unset <https://github.com/rust-lang/libc/pull/4125>
- CI: Ensure there is a fallback for no_std <https://github.com/rust-lang/libc/pull/4125>
- CI: Fix cases where unset variables cause errors <https://github.com/rust-lang/libc/pull/4108>
- CI: Naming adjustments and cleanup <https://github.com/rust-lang/libc/pull/4124>
- CI: Only invoke rustup if running in CI <https://github.com/rust-lang/libc/pull/4107>
- CI: Remove the logic to handle old rust versions <https://github.com/rust-lang/libc/pull/4068>
- CI: Set -u (error on unset) in all script files <https://github.com/rust-lang/libc/pull/4108>
- CI: add support for `loongarch64-unknown-linux-musl` <https://github.com/rust-lang/libc/pull/4092>
- CI: make `aarch64-apple-darwin` not a nightly-only target <https://github.com/rust-lang/libc/pull/4068>
- CI: run shellcheck on all scripts <https://github.com/rust-lang/libc/pull/4042>
- CI: update musl headers to Linux 6.6 <https://github.com/rust-lang/libc/pull/3921>
- CI: use qemu-sparc64 to run sparc64 tests <https://github.com/rust-lang/libc/pull/4133>
- Drop the `libc_const_extern_fn` conditional <https://github.com/rust-lang/libc/commit/674cc1f47f605038ef1aa2cce8e8bc9dac128276>
- Drop the `libc_underscore_const_names` conditional <https://github.com/rust-lang/libc/commit/f0febd5e2e50b38e05259d3afad3c9783711bcf0>
- Explicitly set the edition to 2015 <https://github.com/rust-lang/libc/pull/4058>
- Introduce a `git-blame-ignore-revs` file <https://github.com/rust-lang/libc/pull/4107>
- Tests: Ignore fields as required on Ubuntu 24.10 <https://github.com/rust-lang/libc/pull/4120>
- Tests: skip `ATF_*` constants for OpenBSD <https://github.com/rust-lang/libc/pull/4088>
- Triagebot: Add an autolabel for CI <https://github.com/rust-lang/libc/pull/4052>

## [0.2.164](https://github.com/rust-lang/libc/compare/0.2.163...0.2.164) - 2024-11-16

### MSRV

This release increases the MSRV of `libc` to 1.63.

### Other

- CI: remove tests with rust < 1.63 <https://github.com/rust-lang/libc/pull/4051>
- MSRV: document the MSRV of the stable channel to be 1.63 <https://github.com/rust-lang/libc/pull/4040>
- MacOS: move ifconf to s_no_extra_traits <https://github.com/rust-lang/libc/pull/4051>

## [0.2.163](https://github.com/rust-lang/libc/compare/0.2.162...0.2.163) - 2024-11-16

### Added

- Aix: add more `dlopen` flags <https://github.com/rust-lang/libc/pull/4044>
- Android: add group calls <https://github.com/rust-lang/libc/pull/3499>
- FreeBSD: add `TCP_FUNCTION_BLK` and `TCP_FUNCTION_ALIAS` <https://github.com/rust-lang/libc/pull/4047>
- Linux: add `confstr` <https://github.com/rust-lang/libc/pull/3612>
- Solarish: add `aio` <https://github.com/rust-lang/libc/pull/4033>
- Solarish: add `arc4random*` <https://github.com/rust-lang/libc/pull/3944>

### Changed

- Emscripten: upgrade emsdk to 3.1.68 <https://github.com/rust-lang/libc/pull/3962>
- Hurd: use more standard types <https://github.com/rust-lang/libc/pull/3733>
- Hurd: use the standard `ssize_t = isize` <https://github.com/rust-lang/libc/pull/4029>
- Solaris: fix `confstr` and `ucontext_t` <https://github.com/rust-lang/libc/pull/4035>

### Other

- CI: add Solaris <https://github.com/rust-lang/libc/pull/4035>
- CI: add `i686-unknown-freebsd` <https://github.com/rust-lang/libc/pull/3997>
- CI: ensure that calls to `sort` do not depend on locale <https://github.com/rust-lang/libc/pull/4026>
- Specify `rust-version` in `Cargo.toml` <https://github.com/rust-lang/libc/pull/4041>

## [0.2.162](https://github.com/rust-lang/libc/compare/0.2.161...0.2.162) - 2024-11-07

### Added

- Android: fix the alignment of `uc_mcontext` on arm64 <https://github.com/rust-lang/libc/pull/3894>
- Apple: add `host_cpu_load_info` <https://github.com/rust-lang/libc/pull/3916>
- ESP-IDF: add a time flag <https://github.com/rust-lang/libc/pull/3993>
- FreeBSD: add the `CLOSE_RANGE_CLOEXEC` flag<https://github.com/rust-lang/libc/pull/3996>
- FreeBSD: fix test errors regarding `__gregset_t` <https://github.com/rust-lang/libc/pull/3995>
- FreeBSD: fix tests on x86 FreeBSD 15 <https://github.com/rust-lang/libc/pull/3948>
- FreeBSD: make `ucontext_t` and `mcontext_t` available on all architectures  <https://github.com/rust-lang/libc/pull/3848>
- Haiku: add `getentropy` <https://github.com/rust-lang/libc/pull/3991>
- Illumos: add `syncfs` <https://github.com/rust-lang/libc/pull/3990>
- Illumos: add some recently-added constants <https://github.com/rust-lang/libc/pull/3999>
- Linux: add `ioctl` flags <https://github.com/rust-lang/libc/pull/3960>
- Linux: add epoll busy polling parameters <https://github.com/rust-lang/libc/pull/3922>
- NuttX: add `pthread_[get/set]name_np` <https://github.com/rust-lang/libc/pull/4003>
- RTEMS: add `arc4random_buf` <https://github.com/rust-lang/libc/pull/3989>
- Trusty OS: add initial support <https://github.com/rust-lang/libc/pull/3942>
- WASIp2: expand socket support <https://github.com/rust-lang/libc/pull/3981>

### Fixed

- Emscripten: don't pass `-lc` <https://github.com/rust-lang/libc/pull/4002>
- Hurd: change `st_fsid` field to `st_dev` <https://github.com/rust-lang/libc/pull/3785>
- Hurd: fix the definition of `utsname` <https://github.com/rust-lang/libc/pull/3992>
- Illumos/Solaris: fix `FNM_CASEFOLD` definition <https://github.com/rust-lang/libc/pull/4004>
- Solaris: fix all tests <https://github.com/rust-lang/libc/pull/3864>

### Other

- CI: Add loongarch64 <https://github.com/rust-lang/libc/pull/4000>
- CI: Check that semver files are sorted <https://github.com/rust-lang/libc/pull/4018>
- CI: Re-enable the FreeBSD 15 job <https://github.com/rust-lang/libc/pull/3988>
- Clean up imports and `extern crate` usage <https://github.com/rust-lang/libc/pull/3897>
- Convert `mode_t` constants to octal <https://github.com/rust-lang/libc/pull/3634>
- Remove the `wasm32-wasi` target that has been deleted upstream <https://github.com/rust-lang/libc/pull/4013>

## [0.2.161](https://github.com/rust-lang/libc/compare/0.2.160...0.2.161) - 2024-10-17

### Fixed

- OpenBSD: fix `FNM_PATHNAME` and `FNM_NOESCAPE` values <https://github.com/rust-lang/libc/pull/3983>

## [0.2.160](https://github.com/rust-lang/libc/compare/0.2.159...0.2.160) - 2024-10-17

### Added

- Android: add `PR_GET_NAME` and `PR_SET_NAME` <https://github.com/rust-lang/libc/pull/3941>
- Apple: add `F_TRANSFEREXTENTS` <https://github.com/rust-lang/libc/pull/3925>
- Apple: add `mach_error_string` <https://github.com/rust-lang/libc/pull/3913>
- Apple: add additional `pthread` APIs <https://github.com/rust-lang/libc/pull/3846>
- Apple: add the `LOCAL_PEERTOKEN` socket option <https://github.com/rust-lang/libc/pull/3929>
- BSD: add `RTF_*`, `RTA_*`, `RTAX_*`, and `RTM_*` definitions <https://github.com/rust-lang/libc/pull/3714>
- Emscripten: add `AT_EACCESS` <https://github.com/rust-lang/libc/pull/3911>
- Emscripten: add `getgrgid`, `getgrnam`, `getgrnam_r` and `getgrgid_r` <https://github.com/rust-lang/libc/pull/3912>
- Emscripten: add `getpwnam_r` and `getpwuid_r` <https://github.com/rust-lang/libc/pull/3906>
- FreeBSD: add `POLLRDHUP` <https://github.com/rust-lang/libc/pull/3936>
- Haiku: add `arc4random` <https://github.com/rust-lang/libc/pull/3945>
- Illumos: add `ptsname_r` <https://github.com/rust-lang/libc/pull/3867>
- Linux: add `fanotify` interfaces <https://github.com/rust-lang/libc/pull/3695>
- Linux: add `tcp_info` <https://github.com/rust-lang/libc/pull/3480>
- Linux: add additional AF_PACKET options <https://github.com/rust-lang/libc/pull/3540>
- Linux: make Elf constants always available <https://github.com/rust-lang/libc/pull/3938>
- Musl x86: add `iopl` and `ioperm` <https://github.com/rust-lang/libc/pull/3720>
- Musl: add `posix_spawn` chdir functions <https://github.com/rust-lang/libc/pull/3949>
- Musl: add `utmpx.h` constants <https://github.com/rust-lang/libc/pull/3908>
- NetBSD: add `sysctlnametomib`, `CLOCK_THREAD_CPUTIME_ID` and `CLOCK_PROCESS_CPUTIME_ID` <https://github.com/rust-lang/libc/pull/3927>
- Nuttx: initial support <https://github.com/rust-lang/libc/pull/3909>
- RTEMS: add `getentropy` <https://github.com/rust-lang/libc/pull/3973>
- RTEMS: initial support <https://github.com/rust-lang/libc/pull/3866>
- Solarish: add `POLLRDHUP`, `POSIX_FADV_*`, `O_RSYNC`, and `posix_fallocate` <https://github.com/rust-lang/libc/pull/3936>
- Unix: add `fnmatch.h` <https://github.com/rust-lang/libc/pull/3937>
- VxWorks: add riscv64 support <https://github.com/rust-lang/libc/pull/3935>
- VxWorks: update constants related to the scheduler  <https://github.com/rust-lang/libc/pull/3963>

### Changed

- Redox: change `ino_t` to be `c_ulonglong` <https://github.com/rust-lang/libc/pull/3919>

### Fixed

- ESP-IDF: fix mismatched constants and structs <https://github.com/rust-lang/libc/pull/3920>
- FreeBSD: fix `struct stat` on FreeBSD 12+ <https://github.com/rust-lang/libc/pull/3946>

### Other

- CI: Fix CI for FreeBSD 15 <https://github.com/rust-lang/libc/pull/3950>
- Docs: link to `windows-sys` <https://github.com/rust-lang/libc/pull/3915>

## [0.2.159](https://github.com/rust-lang/libc/compare/0.2.158...0.2.159) - 2024-09-24

### Added

- Android: add more `AT_*` constants in <https://github.com/rust-lang/libc/pull/3779>
- Apple: add missing `NOTE_*` constants in <https://github.com/rust-lang/libc/pull/3883>
- Hermit: add missing error numbers in <https://github.com/rust-lang/libc/pull/3858>
- Hurd: add `__timeval` for 64-bit support in <https://github.com/rust-lang/libc/pull/3786>
- Linux: add `epoll_pwait2` in <https://github.com/rust-lang/libc/pull/3868>
- Linux: add `mq_notify` in <https://github.com/rust-lang/libc/pull/3849>
- Linux: add missing `NFT_CT_*` constants in <https://github.com/rust-lang/libc/pull/3844>
- Linux: add the `fchmodat2` syscall in <https://github.com/rust-lang/libc/pull/3588>
- Linux: add the `mseal` syscall in <https://github.com/rust-lang/libc/pull/3798>
- OpenBSD: add `sendmmsg` and `recvmmsg` in <https://github.com/rust-lang/libc/pull/3831>
- Unix: add `IN6ADDR_ANY_INIT` and `IN6ADDR_LOOPBACK_INIT` in <https://github.com/rust-lang/libc/pull/3693>
- VxWorks: add `S_ISVTX` in <https://github.com/rust-lang/libc/pull/3768>
- VxWorks: add `vxCpuLib` and `taskLib` functions <https://github.com/rust-lang/libc/pull/3861>
- WASIp2: add definitions for `std::net` support in <https://github.com/rust-lang/libc/pull/3892>

### Fixed

- Correctly handle version checks when `clippy-driver` is used <https://github.com/rust-lang/libc/pull/3893>

### Changed

- EspIdf: change signal constants to c_int in <https://github.com/rust-lang/libc/pull/3895>
- HorizonOS: update network definitions in <https://github.com/rust-lang/libc/pull/3863>
- Linux: combine `ioctl` APIs in <https://github.com/rust-lang/libc/pull/3722>
- WASI: enable CI testing in <https://github.com/rust-lang/libc/pull/3869>
- WASIp2: enable CI testing in <https://github.com/rust-lang/libc/pull/3870>

## [0.2.158](https://github.com/rust-lang/libc/compare/0.2.157...0.2.158) - 2024-08-19

### Other
- WASI: fix missing `Iterator` with `rustc-dep-of-std` in <https://github.com/rust-lang/libc/pull/3856#event-13924913068>

## [0.2.157](https://github.com/rust-lang/libc/compare/0.2.156...0.2.157) - 2024-08-17

### Added

- Apple: add `_NSGetArgv`, `_NSGetArgc` and `_NSGetProgname` in <https://github.com/rust-lang/libc/pull/3702>
- Build: add `RUSTC_WRAPPER` support in <https://github.com/rust-lang/libc/pull/3845>
- FreeBSD: add `execvpe` support from 14.1 release in <https://github.com/rust-lang/libc/pull/3745>
- Fuchsia: add `SO_BINDTOIFINDEX`
- Linux: add `klogctl` in <https://github.com/rust-lang/libc/pull/3777>
- MacOS: add `fcntl` OFD commands in <https://github.com/rust-lang/libc/pull/3563>
- NetBSD: add `_lwp_park` in <https://github.com/rust-lang/libc/pull/3721>
- Solaris: add missing networking support in <https://github.com/rust-lang/libc/pull/3717>
- Unix: add `pthread_equal` in <https://github.com/rust-lang/libc/pull/3773>
- WASI: add `select`, `FD_SET`, `FD_ZERO`, `FD_ISSET ` in <https://github.com/rust-lang/libc/pull/3681>

### Fixed
- TEEOS: fix octal notation for `O_*` constants in <https://github.com/rust-lang/libc/pull/3841>

### Changed
- FreeBSD: always use freebsd12 when `rustc_dep_of_std` is set in <https://github.com/rust-lang/libc/pull/3723>

## [0.2.156](https://github.com/rust-lang/libc/compare/v0.2.155...v0.2.156) - 2024-08-15

### Added
- Apple: add `F_ALLOCATEPERSIST` in <https://github.com/rust-lang/libc/pull/3712>
- Apple: add `os_sync_wait_on_address` and related definitions in <https://github.com/rust-lang/libc/pull/3769>
- BSD: generalise `IPV6_DONTFRAG` to all BSD targets in <https://github.com/rust-lang/libc/pull/3716>
- FreeBSD/DragonFly: add `IP_RECVTTL`/`IPV6_RECVHOPLIMIT` in <https://github.com/rust-lang/libc/pull/3751>
- Hurd: add `XATTR_CREATE`, `XATTR_REPLACE` in <https://github.com/rust-lang/libc/pull/3739>
- Linux GNU: `confstr` API and `_CS_*` in <https://github.com/rust-lang/libc/pull/3771>
- Linux musl: add `preadv2` and `pwritev2` (1.2.5 min.) in <https://github.com/rust-lang/libc/pull/3762>
- VxWorks: add the constant `SOMAXCONN` in <https://github.com/rust-lang/libc/pull/3761>
- VxWorks: add a few errnoLib related constants in <https://github.com/rust-lang/libc/pull/3780>

### Fixed
- Solaris/illumos: Change `ifa_flags` type to u64 in <https://github.com/rust-lang/libc/pull/3729>
- QNX 7.0: Disable `libregex` in <https://github.com/rust-lang/libc/pull/3775>

### Changed
- QNX NTO: update platform support in <https://github.com/rust-lang/libc/pull/3815>
- `addr_of!(EXTERN_STATIC)` is now considered safe in <https://github.com/rust-lang/libc/pull/3776>

### Removed
- Apple: remove `rmx_state` in <https://github.com/rust-lang/libc/pull/3776>

### Other
- Update or remove CI tests that have been failing
