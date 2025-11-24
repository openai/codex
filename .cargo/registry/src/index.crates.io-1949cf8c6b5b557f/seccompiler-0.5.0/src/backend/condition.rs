// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use super::{Error, Result, SeccompCmpArgLen, SeccompCmpOp};
use crate::backend::bpf::*;

/// Condition that a syscall must match in order to satisfy a rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeccompCondition {
    /// Index of the argument that is to be compared.
    arg_index: u8,
    /// Length of the argument value that is to be compared.
    arg_len: SeccompCmpArgLen,
    /// Comparison operator to perform.
    operator: SeccompCmpOp,
    /// The value that will be compared with the argument value of the syscall.
    value: u64,
}

impl SeccompCondition {
    /// Creates a new [`SeccompCondition`].
    ///
    /// # Arguments
    ///
    /// * `arg_index` - Index of the argument that is to be compared.
    /// * `arg_len` - Length of the argument value that is to be compared.
    /// * `operator` - Comparison operator to perform.
    /// * `value` - The value that will be compared with the argument value of the syscall.
    ///
    /// # Example
    ///
    /// ```
    /// use seccompiler::{SeccompCmpArgLen, SeccompCmpOp, SeccompCondition};
    ///
    /// let condition = SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, 1).unwrap();
    /// ```
    ///
    /// [`SeccompCondition`]: struct.SeccompCondition.html
    pub fn new(
        arg_index: u8,
        arg_len: SeccompCmpArgLen,
        operator: SeccompCmpOp,
        value: u64,
    ) -> Result<Self> {
        let instance = Self {
            arg_index,
            arg_len,
            operator,
            value,
        };

        instance.validate().map(|_| Ok(instance))?
    }
    /// Validates the `SeccompCondition` data.
    ///
    /// [`SeccompCondition`]: struct.SeccompCondition.html
    fn validate(&self) -> Result<()> {
        // Checks that the given argument number is valid.
        if self.arg_index > ARG_NUMBER_MAX {
            return Err(Error::InvalidArgumentNumber);
        }

        Ok(())
    }

    /// Computes the offsets of the syscall argument data passed to the BPF program.
    ///
    /// Returns the offsets of the most significant and least significant halves of the argument
    /// specified by `arg_index` relative to `struct seccomp_data` passed to the BPF program by
    /// the kernel.
    fn get_data_offsets(&self) -> (u8, u8) {
        // Offset to the argument specified by `arg_index`.
        // Cannot overflow because the value will be at most 16 + 5 * 8 = 56.
        let arg_offset = SECCOMP_DATA_ARGS_OFFSET + self.arg_index * SECCOMP_DATA_ARG_SIZE;

        // Extracts offsets of most significant and least significant halves of argument.
        // Addition cannot overflow because it's at most `arg_offset` + 4 = 68.
        (arg_offset + SECCOMP_DATA_ARG_SIZE / 2, arg_offset)
    }

    /// Splits the `value` field into 32 bit chunks
    ///
    /// Returns the most significant and least significant halves of the `value`.
    fn split_value(&self) -> (u32, u32) {
        ((self.value >> 32) as u32, self.value as u32)
    }

    /// Translates the `eq` (equal) condition into BPF statements.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    fn into_eq_bpf(self, offset: u8) -> Vec<sock_filter> {
        let (msb, lsb) = self.split_value();
        let (msb_offset, lsb_offset) = self.get_data_offsets();

        let mut bpf = match self.arg_len {
            SeccompCmpArgLen::Dword => vec![],
            SeccompCmpArgLen::Qword => vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(msb_offset)),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, msb, 0, offset + 2),
            ],
        };

        bpf.append(&mut vec![
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(lsb_offset)),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, lsb, 0, offset),
        ]);
        bpf
    }

    /// Translates the `ne` (not equal) condition into BPF statements.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    fn into_ne_bpf(self, offset: u8) -> Vec<sock_filter> {
        let (msb, lsb) = self.split_value();
        let (msb_offset, lsb_offset) = self.get_data_offsets();

        let mut bpf = match self.arg_len {
            SeccompCmpArgLen::Dword => vec![],
            SeccompCmpArgLen::Qword => vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(msb_offset)),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, msb, 0, 2),
            ],
        };

        bpf.append(&mut vec![
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(lsb_offset)),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, lsb, offset, 0),
        ]);
        bpf
    }

    /// Translates the `ge` (greater than or equal) condition into BPF statements.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    fn into_ge_bpf(self, offset: u8) -> Vec<sock_filter> {
        let (msb, lsb) = self.split_value();
        let (msb_offset, lsb_offset) = self.get_data_offsets();

        let mut bpf = match self.arg_len {
            SeccompCmpArgLen::Dword => vec![],
            SeccompCmpArgLen::Qword => vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(msb_offset)),
                bpf_jump(BPF_JMP | BPF_JGT | BPF_K, msb, 3, 0),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, msb, 0, offset + 2),
            ],
        };

        bpf.append(&mut vec![
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(lsb_offset)),
            bpf_jump(BPF_JMP | BPF_JGE | BPF_K, lsb, 0, offset),
        ]);
        bpf
    }

    /// Translates the `gt` (greater than) condition into BPF statements.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    fn into_gt_bpf(self, offset: u8) -> Vec<sock_filter> {
        let (msb, lsb) = self.split_value();
        let (msb_offset, lsb_offset) = self.get_data_offsets();

        let mut bpf = match self.arg_len {
            SeccompCmpArgLen::Dword => vec![],
            SeccompCmpArgLen::Qword => vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(msb_offset)),
                bpf_jump(BPF_JMP | BPF_JGT | BPF_K, msb, 3, 0),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, msb, 0, offset + 2),
            ],
        };

        bpf.append(&mut vec![
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(lsb_offset)),
            bpf_jump(BPF_JMP | BPF_JGT | BPF_K, lsb, 0, offset),
        ]);
        bpf
    }

    /// Translates the `le` (less than or equal) condition into BPF statements.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    fn into_le_bpf(self, offset: u8) -> Vec<sock_filter> {
        let (msb, lsb) = self.split_value();
        let (msb_offset, lsb_offset) = self.get_data_offsets();

        let mut bpf = match self.arg_len {
            SeccompCmpArgLen::Dword => vec![],
            SeccompCmpArgLen::Qword => vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(msb_offset)),
                bpf_jump(BPF_JMP | BPF_JGT | BPF_K, msb, offset + 3, 0),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, msb, 0, 2),
            ],
        };

        bpf.append(&mut vec![
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(lsb_offset)),
            bpf_jump(BPF_JMP | BPF_JGT | BPF_K, lsb, offset, 0),
        ]);
        bpf
    }

    /// Translates the `lt` (less than) condition into BPF statements.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    fn into_lt_bpf(self, offset: u8) -> Vec<sock_filter> {
        let (msb, lsb) = self.split_value();
        let (msb_offset, lsb_offset) = self.get_data_offsets();

        let mut bpf = match self.arg_len {
            SeccompCmpArgLen::Dword => vec![],
            SeccompCmpArgLen::Qword => vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(msb_offset)),
                bpf_jump(BPF_JMP | BPF_JGT | BPF_K, msb, offset + 3, 0),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, msb, 0, 2),
            ],
        };

        bpf.append(&mut vec![
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(lsb_offset)),
            bpf_jump(BPF_JMP | BPF_JGE | BPF_K, lsb, offset, 0),
        ]);
        bpf
    }

    /// Translates the `masked_eq` (masked equal) condition into BPF statements.
    ///
    /// The `masked_eq` condition is `true` if the result of logical `AND` between the given value
    /// and the mask is the value being compared against.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    fn into_masked_eq_bpf(mut self, offset: u8, mask: u64) -> Vec<sock_filter> {
        // Mask the current value.
        self.value &= mask;

        let (msb_offset, lsb_offset) = self.get_data_offsets();
        let (msb, lsb) = self.split_value();
        let (mask_msb, mask_lsb) = ((mask >> 32) as u32, mask as u32);

        let mut bpf = match self.arg_len {
            SeccompCmpArgLen::Dword => vec![],
            SeccompCmpArgLen::Qword => vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(msb_offset)),
                bpf_stmt(BPF_ALU | BPF_AND | BPF_K, mask_msb),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, msb, 0, offset + 3),
            ],
        };

        bpf.append(&mut vec![
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, u32::from(lsb_offset)),
            bpf_stmt(BPF_ALU | BPF_AND | BPF_K, mask_lsb),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, lsb, 0, offset),
        ]);
        bpf
    }

    /// Translates the [`SeccompCondition`] into BPF statements.
    ///
    /// # Arguments
    ///
    /// * `offset` - The given jump offset to the start of the next rule.
    ///
    /// The jump is performed if the condition fails and thus the current rule does not match so
    /// `seccomp` tries to match the next rule by jumping out of the current rule.
    ///
    /// In case the condition is part of the last rule, the jump offset is to the default action of
    /// respective filter.
    ///
    /// The most significant and least significant halves of the argument value are compared
    /// separately since the BPF operand and accumulator are 4 bytes whereas an argument value is 8.
    pub(crate) fn into_bpf(self, offset: u8) -> Vec<sock_filter> {
        let result = match self.operator {
            SeccompCmpOp::Eq => self.into_eq_bpf(offset),
            SeccompCmpOp::Ge => self.into_ge_bpf(offset),
            SeccompCmpOp::Gt => self.into_gt_bpf(offset),
            SeccompCmpOp::Le => self.into_le_bpf(offset),
            SeccompCmpOp::Lt => self.into_lt_bpf(offset),
            SeccompCmpOp::MaskedEq(mask) => self.into_masked_eq_bpf(offset, mask),
            SeccompCmpOp::Ne => self.into_ne_bpf(offset),
        };

        // Verifies that the `CONDITION_MAX_LEN` constant was properly updated.
        assert!(result.len() <= CONDITION_MAX_LEN as usize);

        result
    }
}

#[cfg(test)]
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;
    #[test]
    fn test_new_condition() {
        assert!(SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, 60).is_ok());
        assert_eq!(
            SeccompCondition::new(7, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, 60).unwrap_err(),
            Error::InvalidArgumentNumber
        );
    }

    #[test]
    fn test_get_data_offsets() {
        let cond = SeccompCondition::new(1, SeccompCmpArgLen::Qword, SeccompCmpOp::Eq, 60).unwrap();
        let (msb_offset, lsb_offset) = cond.get_data_offsets();
        assert_eq!(
            (msb_offset, lsb_offset),
            (
                SECCOMP_DATA_ARGS_OFFSET + SECCOMP_DATA_ARG_SIZE + 4,
                SECCOMP_DATA_ARGS_OFFSET + SECCOMP_DATA_ARG_SIZE
            )
        );

        let data = libc::seccomp_data {
            nr: 0,
            arch: 0,
            instruction_pointer: AUDIT_ARCH_X86_64 as u64,
            args: [
                u64::MAX,
                u32::MAX as u64 + 1,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
            ],
        };
        let data_ptr = (&data as *const libc::seccomp_data) as *const u32;

        assert_eq!(unsafe { *data_ptr.offset((lsb_offset / 4) as isize) }, 0);
        assert_eq!(unsafe { *data_ptr.offset((msb_offset / 4) as isize) }, 1);
    }

    #[test]
    fn test_split_value() {
        let cond = SeccompCondition::new(
            1,
            SeccompCmpArgLen::Qword,
            SeccompCmpOp::Eq,
            u32::MAX as u64 + 1,
        )
        .unwrap();
        assert_eq!(cond.split_value(), (1, 0));
    }
}
