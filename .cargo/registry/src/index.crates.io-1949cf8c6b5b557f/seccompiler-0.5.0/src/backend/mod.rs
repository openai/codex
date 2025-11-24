// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! This module defines the data structures used for the intermmediate representation (IR),
//! as well as the logic for compiling the filter into BPF code, the final form of the filter.

mod bpf;
mod condition;
mod filter;
mod rule;

pub use condition::SeccompCondition;
pub use filter::SeccompFilter;
pub use rule::SeccompRule;

#[cfg(feature = "json")]
use serde::Deserialize;

use core::fmt::Formatter;
use std::fmt::Display;

// See /usr/include/linux/seccomp.h
use libc::{
    SECCOMP_RET_ALLOW, SECCOMP_RET_DATA, SECCOMP_RET_ERRNO, SECCOMP_RET_KILL_PROCESS,
    SECCOMP_RET_KILL_THREAD, SECCOMP_RET_LOG, SECCOMP_RET_TRACE, SECCOMP_RET_TRAP,
};

use bpf::{ARG_NUMBER_MAX, AUDIT_ARCH_AARCH64, AUDIT_ARCH_RISCV64, AUDIT_ARCH_X86_64, BPF_MAX_LEN};

pub use bpf::{sock_filter, BpfProgram, BpfProgramRef};

/// Backend Result type.
type Result<T> = std::result::Result<T, Error>;

/// Backend-related errors.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Attempting to associate an empty vector of conditions to a rule.
    EmptyRule,
    /// Filter exceeds the maximum number of instructions that a BPF program can have.
    FilterTooLarge(usize),
    /// Filter and default actions are equal.
    IdenticalActions,
    /// Argument index of a `SeccompCondition` exceeds the maximum linux syscall index.
    InvalidArgumentNumber,
    /// Invalid TargetArch.
    InvalidTargetArch(String),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        use self::Error::*;

        match self {
            EmptyRule => {
                write!(f, "The condition vector of a rule cannot be empty.")
            }
            FilterTooLarge(len) => write!(
                f,
                "The seccomp filter contains too many BPF instructions: {}. Max length is {}.",
                len, BPF_MAX_LEN
            ),
            IdenticalActions => write!(f, "`match_action` and `mismatch_action` are equal."),
            InvalidArgumentNumber => {
                write!(
                    f,
                    "The seccomp rule contains an invalid argument index. Maximum index value: {}",
                    ARG_NUMBER_MAX
                )
            }
            InvalidTargetArch(arch) => write!(f, "Invalid target arch: {}.", arch),
        }
    }
}

/// Supported target architectures.
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TargetArch {
    /// x86_64 arch
    x86_64,
    /// aarch64 arch
    aarch64,
    /// riscv64 arch
    riscv64,
}

impl TargetArch {
    /// Get the arch audit value. Used for the runtime arch check embedded in the BPF filter.
    fn get_audit_value(self) -> u32 {
        match self {
            TargetArch::x86_64 => AUDIT_ARCH_X86_64,
            TargetArch::aarch64 => AUDIT_ARCH_AARCH64,
            TargetArch::riscv64 => AUDIT_ARCH_RISCV64,
        }
    }
}

impl TryFrom<&str> for TargetArch {
    type Error = Error;
    fn try_from(input: &str) -> Result<Self> {
        match input.to_lowercase().as_str() {
            "x86_64" => Ok(TargetArch::x86_64),
            "aarch64" => Ok(TargetArch::aarch64),
            "riscv64" => Ok(TargetArch::riscv64),
            _ => Err(Error::InvalidTargetArch(input.to_string())),
        }
    }
}

/// Comparison to perform when matching a condition.
#[cfg_attr(
    feature = "json",
    derive(Deserialize),
    serde(rename_all = "snake_case")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeccompCmpOp {
    /// Argument value is equal to the specified value.
    Eq,
    /// Argument value is greater than or equal to the specified value.
    Ge,
    /// Argument value is greater than specified value.
    Gt,
    /// Argument value is less than or equal to the specified value.
    Le,
    /// Argument value is less than specified value.
    Lt,
    /// Masked bits of argument value are equal to masked bits of specified value.
    MaskedEq(u64),
    /// Argument value is not equal to specified value.
    Ne,
}

/// Seccomp argument value length.
#[cfg_attr(feature = "json", derive(Deserialize), serde(rename_all = "lowercase"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeccompCmpArgLen {
    /// Argument value length is 4 bytes.
    Dword,
    /// Argument value length is 8 bytes.
    Qword,
}

/// Actions that a seccomp filter can return for a syscall.
#[cfg_attr(
    feature = "json",
    derive(Deserialize),
    serde(rename_all = "snake_case")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeccompAction {
    /// Allows syscall.
    Allow,
    /// Returns from syscall with specified error number.
    Errno(u32),
    /// Kills calling thread.
    KillThread,
    /// Kills calling process.
    KillProcess,
    /// Allows syscall after logging it.
    Log,
    /// Notifies tracing process of the caller with respective number.
    Trace(u32),
    /// Sends `SIGSYS` to the calling process.
    Trap,
}

impl From<SeccompAction> for u32 {
    /// Return codes of the BPF program for each action.
    ///
    /// # Arguments
    ///
    /// * `action` - The [`SeccompAction`] that the kernel will take.
    ///
    /// [`SeccompAction`]: enum.SeccompAction.html
    fn from(action: SeccompAction) -> Self {
        match action {
            SeccompAction::Allow => SECCOMP_RET_ALLOW,
            SeccompAction::Errno(x) => SECCOMP_RET_ERRNO | (x & SECCOMP_RET_DATA),
            SeccompAction::KillThread => SECCOMP_RET_KILL_THREAD,
            SeccompAction::KillProcess => SECCOMP_RET_KILL_PROCESS,
            SeccompAction::Log => SECCOMP_RET_LOG,
            SeccompAction::Trace(x) => SECCOMP_RET_TRACE | (x & SECCOMP_RET_DATA),
            SeccompAction::Trap => SECCOMP_RET_TRAP,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_arch() {
        assert!(TargetArch::try_from("invalid").is_err());
        assert!(TargetArch::try_from("x8664").is_err());

        assert_eq!(TargetArch::try_from("x86_64").unwrap(), TargetArch::x86_64);
        assert_eq!(TargetArch::try_from("X86_64").unwrap(), TargetArch::x86_64);

        assert_eq!(
            TargetArch::try_from("aarch64").unwrap(),
            TargetArch::aarch64
        );
        assert_eq!(
            TargetArch::try_from("aARch64").unwrap(),
            TargetArch::aarch64
        );

        assert_eq!(
            TargetArch::try_from("riscv64").unwrap(),
            TargetArch::riscv64
        );
        assert_eq!(
            TargetArch::try_from("RiScV64").unwrap(),
            TargetArch::riscv64
        );
    }
}
