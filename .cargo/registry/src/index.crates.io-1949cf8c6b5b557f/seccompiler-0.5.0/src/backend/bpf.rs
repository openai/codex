// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Define constants, helpers and types used for BPF codegen.

use super::TargetArch;

/// Program made up of a sequence of BPF instructions.
pub type BpfProgram = Vec<sock_filter>;

/// Reference to program made up of a sequence of BPF instructions.
pub type BpfProgramRef<'a> = &'a [sock_filter];

// The maximum number of a syscall argument.
// A syscall can have at most 6 arguments.
// Arguments are numbered from 0 to 5.
pub const ARG_NUMBER_MAX: u8 = 5;

// The maximum number of BPF statements that a condition will be translated into.
// This is not a linux requirement but is based on the current BPF compilation logic.
// This is used in the backend code for preallocating vectors and for detecting unjumpable offsets.
pub const CONDITION_MAX_LEN: u8 = 6;

// The maximum seccomp-BPF program length allowed by the linux kernel.
pub const BPF_MAX_LEN: usize = 4096;

// `struct seccomp_data` offsets and sizes of fields in bytes:
//
// ```c
// struct seccomp_data {
//     int nr;
//     __u32 arch;
//     __u64 instruction_pointer;
//     __u64 args[6];
// };
// ```
pub const SECCOMP_DATA_NR_OFFSET: u8 = 0;
const SECCOMP_DATA_ARCH_OFFSET: u8 = 4;
pub const SECCOMP_DATA_ARGS_OFFSET: u8 = 16;
pub const SECCOMP_DATA_ARG_SIZE: u8 = 8;

// Builds a `jump` BPF instruction.
//
// # Arguments
//
// * `code` - The operation code.
// * `jt` - The jump offset in case the operation returns `true`.
// * `jf` - The jump offset in case the operation returns `false`.
// * `k` - The operand.
#[inline(always)]
pub(crate) fn bpf_jump(code: u16, k: u32, jt: u8, jf: u8) -> sock_filter {
    sock_filter { code, jt, jf, k }
}

// Builds a "statement" BPF instruction.
//
// # Arguments
//
// * `code` - The operation code.
// * `k` - The operand.
#[inline(always)]
pub(crate) fn bpf_stmt(code: u16, k: u32) -> sock_filter {
    sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    }
}

// Builds a sequence of BPF instructions that validate the underlying architecture.
#[inline(always)]
pub(crate) fn build_arch_validation_sequence(target_arch: TargetArch) -> Vec<sock_filter> {
    let audit_arch_value = target_arch.get_audit_value();
    vec![
        bpf_stmt(BPF_LD | BPF_W | BPF_ABS, SECCOMP_DATA_ARCH_OFFSET as u32),
        bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, audit_arch_value, 1, 0),
        bpf_stmt(BPF_RET | BPF_K, libc::SECCOMP_RET_KILL_PROCESS),
    ]
}

// BPF Instruction classes.
// See /usr/include/linux/bpf_common.h .
// Load operation.
pub const BPF_LD: u16 = 0x00;
// ALU operation.
pub const BPF_ALU: u16 = 0x04;
// Jump operation.
pub const BPF_JMP: u16 = 0x05;
// Return operation.
pub const BPF_RET: u16 = 0x06;

// BPF ld/ldx fields.
// See /usr/include/linux/bpf_common.h .
// Operand size is a word.
pub const BPF_W: u16 = 0x00;
// Load from data area (where `seccomp_data` is).
pub const BPF_ABS: u16 = 0x20;

// BPF alu fields.
// See /usr/include/linux/bpf_common.h .
pub const BPF_AND: u16 = 0x50;

// BPF jmp fields.
// See /usr/include/linux/bpf_common.h .
// Unconditional jump.
pub const BPF_JA: u16 = 0x00;
// Jump with comparisons.
pub const BPF_JEQ: u16 = 0x10;
pub const BPF_JGT: u16 = 0x20;
pub const BPF_JGE: u16 = 0x30;
// Test against the value in the K register.
pub const BPF_K: u16 = 0x00;

// Architecture identifier for x86_64 LE.
// See /usr/include/linux/audit.h .
// Defined as:
// `#define AUDIT_ARCH_X86_64	(EM_X86_64|__AUDIT_ARCH_64BIT|__AUDIT_ARCH_LE)`
pub const AUDIT_ARCH_X86_64: u32 = 62 | 0x8000_0000 | 0x4000_0000;

// Architecture identifier for aarch64 LE.
// Defined as:
// `#define AUDIT_ARCH_AARCH64	(EM_AARCH64|__AUDIT_ARCH_64BIT|__AUDIT_ARCH_LE)`
pub const AUDIT_ARCH_AARCH64: u32 = 183 | 0x8000_0000 | 0x4000_0000;

// Architecture identifier for riscv64 LE.
// Defined as:
// `#define AUDIT_ARCH_RISCV64  (EM_RISCV|__AUDIT_ARCH_64BIT|__AUDIT_ARCH_LE)`
pub const AUDIT_ARCH_RISCV64: u32 = 243 | 0x8000_0000 | 0x4000_0000;

/// BPF instruction structure definition.
// See /usr/include/linux/filter.h .
// We cannot use the `libc::sock_filter` definition since it doesn't implement `Debug` and
// `PartialEq`.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct sock_filter {
    /// Code of the instruction.
    pub code: ::std::os::raw::c_ushort,
    /// Jump if true offset.
    pub jt: ::std::os::raw::c_uchar,
    /// Jump if false offset.
    pub jf: ::std::os::raw::c_uchar,
    /// Immediate value.
    pub k: ::std::os::raw::c_uint,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpf_functions() {
        // Compares the output of the BPF instruction generating functions to hardcoded
        // instructions.
        assert_eq!(
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 16),
            sock_filter {
                code: 0x20,
                jt: 0,
                jf: 0,
                k: 16,
            }
        );
        assert_eq!(
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 10, 2, 5),
            sock_filter {
                code: 0x15,
                jt: 2,
                jf: 5,
                k: 10,
            }
        );
    }
}
