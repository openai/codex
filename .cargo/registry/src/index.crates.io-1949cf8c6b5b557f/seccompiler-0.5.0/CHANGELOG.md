# Upcoming Release

# v0.5.0

## Added
- [[#72]](https://github.com/rust-vmm/seccompiler/pull/72): Introduce RISC-V
  64-bit architecture support.

## Changed
- [[#78]](https://github.com/rust-vmm/seccompiler/pull/78): Update
  `syscall_tables` from v6.12 kernel source

# v0.4.0

## Changed
- Seccomp is now activated via the seccomp syscall, not prctl

## Added
- A new Error::Seccomp variant is added to indictate seccomp syscall failures
- Add `apply_filter_all_threads` convenience function which uses the seccomp
  TSYNC feature to synchronize all threads in the process to the same filter
- A new Error::ThreadSync variant is added to indicate failure to sync threads

# v0.3.0

## Changed
- [[#40]](https://github.com/rust-vmm/seccompiler/pull/40): Update Rust
  to Edition 2021

## Fixed

- [[#31]](https://github.com/rust-vmm/seccompiler/issues/31): Implement
  `From<BackendError>` for `Error`
- [[#40]](https://github.com/rust-vmm/seccompiler/pull/40): Fix clippy
  complaints about missing `Eq` when `PartialEq` is implemented

# v0.2.0

First release

## Added

- [[#7]](https://github.com/rust-vmm/seccompiler/pull/7): Add functionality for
  Rust-based filters and implement the BPF compilation logic.
- [[#9]](https://github.com/rust-vmm/seccompiler/pull/9): Add json frontend,
  gated by the `json` feature.
