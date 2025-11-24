// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

mod aarch64;
mod riscv64;
mod x86_64;

use crate::backend::TargetArch;
use std::collections::HashMap;

/// Creates and owns a mapping from the arch-specific syscall name to the right number.
#[derive(Debug)]
pub(crate) struct SyscallTable {
    map: HashMap<&'static str, i64>,
}

impl SyscallTable {
    pub fn new(arch: TargetArch) -> Self {
        Self {
            map: match arch {
                TargetArch::aarch64 => aarch64::make_syscall_table(),
                TargetArch::x86_64 => x86_64::make_syscall_table(),
                TargetArch::riscv64 => riscv64::make_syscall_table(),
            },
        }
    }

    /// Returns the arch-specific syscall number based on the given name.
    pub fn get_syscall_nr(&self, sys_name: &str) -> Option<i64> {
        self.map.get(sys_name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::SyscallTable;
    use crate::backend::TargetArch;

    #[test]
    fn test_get_syscall_nr() {
        // get number for a valid syscall
        let instance_x86_64 = SyscallTable::new(TargetArch::x86_64);
        let instance_aarch64 = SyscallTable::new(TargetArch::aarch64);
        let instance_riscv64 = SyscallTable::new(TargetArch::riscv64);

        assert_eq!(instance_x86_64.get_syscall_nr("close").unwrap(), 3);
        assert_eq!(instance_aarch64.get_syscall_nr("close").unwrap(), 57);
        assert_eq!(instance_riscv64.get_syscall_nr("close").unwrap(), 57);

        // invalid syscall name
        assert!(instance_x86_64.get_syscall_nr("nosyscall").is_none());
        assert!(instance_aarch64.get_syscall_nr("nosyscall").is_none());
        assert!(instance_riscv64.get_syscall_nr("nosyscall").is_none());
    }
}
