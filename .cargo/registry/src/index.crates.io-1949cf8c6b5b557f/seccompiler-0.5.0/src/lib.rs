// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
#![deny(missing_docs)]
#![cfg(target_endian = "little")]
//! Provides easy-to-use Linux seccomp-bpf jailing.
//!
//! Seccomp is a Linux kernel security feature which enables a tight control over what kernel-level
//! mechanisms a process has access to. This is typically used to reduce the attack surface and
//! exposed resources when running untrusted code. This works by allowing users to write and set a
//! BPF (Berkeley Packet Filter) program for each process or thread, that intercepts syscalls and
//! decides whether the syscall is safe to execute.
//!
//! Writing BPF programs by hand is difficult and error-prone. This crate provides high-level
//! wrappers for working with system call filtering.
//!
//! The core concept of the library is the filter. It is an abstraction that
//! models a collection of syscall-mapped rules, coupled with on-match and
//! default actions, that logically describes a policy for dispatching actions
//! (e.g. Allow, Trap, Errno) for incoming system calls.
//!
//! Seccompiler provides constructs for defining filters, compiling them into
//! loadable BPF programs and installing them in the kernel.
//!
//! Filters are defined either with a JSON file or using Rust code, with
//! library-defined structures. Both representations are semantically equivalent
//! and model the rules of the filter. Choosing one or the other depends on the use
//! case and preference.
//!
//! # Supported platforms
//!
//! Due to the fact that seccomp is a Linux-specific feature, this crate is
//! supported only on Linux systems.
//!
//! Supported host architectures:
//! - Little-endian x86_64
//! - Little-endian aarch64
//! - Little-endian riscv64
//!
//! # Terminology
//!
//! The smallest unit of the [`SeccompFilter`] is the [`SeccompCondition`], which is a
//! comparison operation applied to the current system call. It’s parametrised by
//! the argument index, the length of the argument, the operator and the actual
//! expected value.
//!
//! Going one step further, a [`SeccompRule`] is a vector of [`SeccompCondition`]s,
//! that must all match for the rule to be considered matched. In other words, a
//! rule is a collection of **and-bound** conditions for a system call.
//!
//! Finally, at the top level, there’s the [`SeccompFilter`]. The filter can be
//! viewed as a collection of syscall-associated rules, with a predefined on-match
//! [`SeccompAction`] and a default [`SeccompAction`] that is returned if none of the rules match.
//!
//! In a filter, each system call number maps to a vector of **or-bound** rules.
//! In order for the filter to match, it is enough that one rule associated to the
//! system call matches. A system call may also map to an empty rule vector, which
//! means that the system call will match, regardless of the actual arguments.
//!
//! # Examples
//!
//! The following example defines and installs a simple Rust filter, that sends SIGSYS for
//! `accept4`, `fcntl(any, F_SETFD, FD_CLOEXEC, ..)` and `fcntl(any, F_GETFD, ...)`.
//! It allows any other syscalls.
//!
//! ```
//! use seccompiler::{
//!     BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
//!     SeccompRule,
//! };
//! use std::convert::TryInto;
//!
//! let filter: BpfProgram = SeccompFilter::new(
//!     vec![
//!         (libc::SYS_accept4, vec![]),
//!         (
//!             libc::SYS_fcntl,
//!             vec![
//!                 SeccompRule::new(vec![
//!                     SeccompCondition::new(
//!                         1,
//!                         SeccompCmpArgLen::Dword,
//!                         SeccompCmpOp::Eq,
//!                         libc::F_SETFD as u64,
//!                     )
//!                     .unwrap(),
//!                     SeccompCondition::new(
//!                         2,
//!                         SeccompCmpArgLen::Dword,
//!                         SeccompCmpOp::Eq,
//!                         libc::FD_CLOEXEC as u64,
//!                     )
//!                     .unwrap(),
//!                 ])
//!                 .unwrap(),
//!                 SeccompRule::new(vec![SeccompCondition::new(
//!                     1,
//!                     SeccompCmpArgLen::Dword,
//!                     SeccompCmpOp::Eq,
//!                     libc::F_GETFD as u64,
//!                 )
//!                 .unwrap()])
//!                 .unwrap(),
//!             ],
//!         ),
//!     ]
//!     .into_iter()
//!     .collect(),
//!     SeccompAction::Allow,
//!     SeccompAction::Trap,
//!     std::env::consts::ARCH.try_into().unwrap(),
//! )
//! .unwrap()
//! .try_into()
//! .unwrap();
//!
//! seccompiler::apply_filter(&filter).unwrap();
//! ```
//!
//!
//! This second example defines and installs an equivalent JSON filter (uses the `json` feature):
//!
//! ```
//! # #[cfg(feature = "json")]
//! # {
//! use seccompiler::BpfMap;
//! use std::convert::TryInto;
//!
//! let json_input = r#"{
//!     "main_thread": {
//!         "mismatch_action": "allow",
//!         "match_action": "trap",
//!         "filter": [
//!             {
//!                 "syscall": "accept4"
//!             },
//!             {
//!                 "syscall": "fcntl",
//!                 "args": [
//!                     {
//!                         "index": 1,
//!                         "type": "dword",
//!                         "op": "eq",
//!                         "val": 2,
//!                         "comment": "F_SETFD"
//!                     },
//!                     {
//!                         "index": 2,
//!                         "type": "dword",
//!                         "op": "eq",
//!                         "val": 1,
//!                         "comment": "FD_CLOEXEC"
//!                     }
//!                 ]
//!             },
//!             {
//!                 "syscall": "fcntl",
//!                 "args": [
//!                     {
//!                         "index": 1,
//!                         "type": "dword",
//!                         "op": "eq",
//!                         "val": 1,
//!                         "comment": "F_GETFD"
//!                     }
//!                 ]
//!             }
//!         ]
//!     }
//! }"#;
//!
//! let filter_map: BpfMap = seccompiler::compile_from_json(
//!     json_input.as_bytes(),
//!     std::env::consts::ARCH.try_into().unwrap(),
//! )
//! .unwrap();
//! let filter = filter_map.get("main_thread").unwrap();
//!
//! seccompiler::apply_filter(&filter).unwrap();
//!
//! # }
//! ```
//!
//! [`SeccompFilter`]: struct.SeccompFilter.html
//! [`SeccompCondition`]: struct.SeccompCondition.html
//! [`SeccompRule`]: struct.SeccompRule.html
//! [`SeccompAction`]: enum.SeccompAction.html

mod backend;
#[cfg(feature = "json")]
mod frontend;
#[cfg(feature = "json")]
mod syscall_table;

#[cfg(feature = "json")]
use std::convert::TryInto;
#[cfg(feature = "json")]
use std::io::Read;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io;

#[cfg(feature = "json")]
use frontend::json::{Error as JsonFrontendError, JsonCompiler};

// Re-export the IR public types.
pub use backend::{
    sock_filter, BpfProgram, BpfProgramRef, Error as BackendError, SeccompAction, SeccompCmpArgLen,
    SeccompCmpOp, SeccompCondition, SeccompFilter, SeccompRule, TargetArch,
};

// BPF structure definition for filter array.
// See /usr/include/linux/filter.h .
#[repr(C)]
struct sock_fprog {
    pub len: ::std::os::raw::c_ushort,
    pub filter: *const sock_filter,
}

/// Library Result type.
pub type Result<T> = std::result::Result<T, Error>;

///`BpfMap` is another type exposed by the library, which maps thread categories to BPF programs.
pub type BpfMap = HashMap<String, BpfProgram>;

/// Library errors.
#[derive(Debug)]
pub enum Error {
    /// Error originating in the backend compiler.
    Backend(BackendError),
    /// Attempting to install an empty filter.
    EmptyFilter,
    /// System error related to calling `prctl`.
    Prctl(io::Error),
    /// System error related to calling `seccomp` syscall.
    Seccomp(io::Error),
    /// Returned when calling `seccomp` with the thread sync flag (TSYNC) fails. Contains the pid
    /// of the thread that caused the failure.
    ThreadSync(libc::c_long),
    /// Json Frontend Error.
    #[cfg(feature = "json")]
    JsonFrontend(JsonFrontendError),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use self::Error::*;

        match self {
            Backend(error) => Some(error),
            Prctl(error) => Some(error),
            Seccomp(error) => Some(error),
            ThreadSync(_) => None,
            #[cfg(feature = "json")]
            JsonFrontend(error) => Some(error),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        use self::Error::*;

        match self {
            Backend(error) => {
                write!(f, "Backend error: {}", error)
            }
            EmptyFilter => {
                write!(f, "Cannot install empty filter.")
            }
            Prctl(errno) => {
                write!(f, "Error calling `prctl`: {}", errno)
            }
            Seccomp(errno) => {
                write!(f, "Error calling `seccomp`: {}", errno)
            }
            ThreadSync(pid) => {
                write!(
                    f,
                    "Seccomp filter synchronization failed in thread `{}`",
                    pid
                )
            }
            #[cfg(feature = "json")]
            JsonFrontend(error) => {
                write!(f, "Json Frontend error: {}", error)
            }
        }
    }
}

impl From<BackendError> for Error {
    fn from(value: BackendError) -> Self {
        Self::Backend(value)
    }
}
#[cfg(feature = "json")]
impl From<JsonFrontendError> for Error {
    fn from(value: JsonFrontendError) -> Self {
        Self::JsonFrontend(value)
    }
}

/// Apply a BPF filter to the calling thread.
///
/// # Arguments
///
/// * `bpf_filter` - A reference to the [`BpfProgram`] to be installed.
///
/// [`BpfProgram`]: type.BpfProgram.html
pub fn apply_filter(bpf_filter: BpfProgramRef) -> Result<()> {
    apply_filter_with_flags(bpf_filter, 0)
}

/// Apply a BPF filter to the all threads in the process via the TSYNC feature. Please read the
/// man page for seccomp (`man 2 seccomp`) for more information.
///
/// # Arguments
///
/// * `bpf_filter` - A reference to the [`BpfProgram`] to be installed.
///
/// [`BpfProgram`]: type.BpfProgram.html
pub fn apply_filter_all_threads(bpf_filter: BpfProgramRef) -> Result<()> {
    apply_filter_with_flags(bpf_filter, libc::SECCOMP_FILTER_FLAG_TSYNC)
}

/// Apply a BPF filter to the calling thread.
///
/// # Arguments
///
/// * `bpf_filter` - A reference to the [`BpfProgram`] to be installed.
/// * `flags` - A u64 representing a bitset of seccomp's flags parameter.
///
/// [`BpfProgram`]: type.BpfProgram.html
fn apply_filter_with_flags(bpf_filter: BpfProgramRef, flags: libc::c_ulong) -> Result<()> {
    // If the program is empty, don't install the filter.
    if bpf_filter.is_empty() {
        return Err(Error::EmptyFilter);
    }

    // SAFETY:
    // Safe because syscall arguments are valid.
    let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(Error::Prctl(io::Error::last_os_error()));
    }

    let bpf_prog = sock_fprog {
        len: bpf_filter.len() as u16,
        filter: bpf_filter.as_ptr(),
    };
    let bpf_prog_ptr = &bpf_prog as *const sock_fprog;

    // SAFETY:
    // Safe because the kernel performs a `copy_from_user` on the filter and leaves the memory
    // untouched. We can therefore use a reference to the BpfProgram, without needing ownership.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            libc::SECCOMP_SET_MODE_FILTER,
            flags,
            bpf_prog_ptr,
        )
    };

    #[allow(clippy::comparison_chain)]
    // Per manpage, if TSYNC fails, retcode is >0 and equals the pid of the thread that caused the
    // failure. Otherwise, error code is -1 and errno is set.
    if rc < 0 {
        return Err(Error::Seccomp(io::Error::last_os_error()));
    } else if rc > 0 {
        return Err(Error::ThreadSync(rc));
    }

    Ok(())
}

/// Compile [`BpfProgram`]s from JSON.
///
/// # Arguments
///
/// * `reader` - [`std::io::Read`] object containing the JSON data conforming to the
///    [JSON file format](https://github.com/rust-vmm/seccompiler/blob/master/docs/json_format.md).
/// * `arch` - target architecture of the filter.
///
/// [`BpfProgram`]: type.BpfProgram.html
#[cfg(feature = "json")]
pub fn compile_from_json<R: Read>(reader: R, arch: TargetArch) -> Result<BpfMap> {
    // Run the frontend.
    let seccomp_filters: HashMap<String, SeccompFilter> =
        JsonCompiler::new(arch).compile(reader)?;

    // Run the backend.
    let mut bpf_data: BpfMap = BpfMap::with_capacity(seccomp_filters.len());
    for (name, seccomp_filter) in seccomp_filters {
        bpf_data.insert(name, seccomp_filter.try_into()?);
    }

    Ok(bpf_data)
}
