// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use crate::backend::{bpf::*, condition::SeccompCondition, Error, Result};

/// Rule that a filter attempts to match for a syscall.
///
/// If all conditions match then rule gets matched.
/// A syscall can have many rules associated. If either of them matches, the `match_action` of the
/// [`SeccompFilter`] is triggered.
///
/// [`SeccompFilter`]: struct.SeccompFilter.html
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeccompRule {
    /// Conditions of rule that need to match in order for the rule to get matched.
    conditions: Vec<SeccompCondition>,
}

impl SeccompRule {
    /// Creates a new rule. Rules with 0 conditions are not allowed; to match a syscall regardless
    /// of argument values, map the syscall number to an empty vector of rules when constructing
    /// the [`SeccompFilter`](super::SeccompFilter) instead.
    ///
    /// # Arguments
    ///
    /// * `conditions` - Vector of [`SeccompCondition`]s that the syscall must match.
    ///
    /// # Example
    ///
    /// ```
    /// use seccompiler::{SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompRule};
    ///
    /// let rule = SeccompRule::new(vec![
    ///     SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, 1).unwrap(),
    ///     SeccompCondition::new(1, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, 1).unwrap(),
    /// ])
    /// .unwrap();
    /// ```
    ///
    /// [`SeccompCondition`]: struct.SeccompCondition.html
    pub fn new(conditions: Vec<SeccompCondition>) -> Result<Self> {
        let instance = Self { conditions };
        instance.validate()?;

        Ok(instance)
    }

    /// Performs semantic checks on the SeccompRule.
    fn validate(&self) -> Result<()> {
        // Rules with no conditions are not allowed. Syscalls mappings to empty rule vectors are to
        // be used instead, for matching only on the syscall number.
        if self.conditions.is_empty() {
            return Err(Error::EmptyRule);
        }

        Ok(())
    }

    /// Appends a condition of the rule to an accumulator.
    ///
    /// The length of the rule and offset to the next rule are updated.
    ///
    /// # Arguments
    ///
    /// * `condition` - The condition added to the rule.
    /// * `accumulator` - Accumulator of BPF statements that compose the BPF program.
    /// * `rule_len` - Number of conditions in the rule.
    /// * `offset` - Offset (in number of BPF statements) to the next rule.
    fn append_condition(
        condition: SeccompCondition,
        accumulator: &mut Vec<Vec<sock_filter>>,
        offset: &mut u8,
    ) {
        // Tries to detect whether prepending the current condition will produce an unjumpable
        // offset (since BPF conditional jumps are a maximum of 255 instructions, which is
        // u8::MAX).
        if offset.checked_add(CONDITION_MAX_LEN + 1).is_none() {
            // If that is the case, three additional helper jumps are prepended and the offset
            // is reset to 1.
            //
            // - The first jump continues the evaluation of the condition chain by jumping to
            //   the next condition or the action of the rule if the last condition was matched.
            // - The second, jumps out of the rule, to the next rule or the default action of
            //   the filter in case of the last rule in the rule chain of a syscall.
            // - The third jumps out of the rule chain of the syscall, to the rule chain of the
            //   next syscall number to be checked or the default action of the filter in the
            //   case of the last rule chain.
            let helper_jumps = vec![
                bpf_stmt(BPF_JMP | BPF_JA, 2),
                bpf_stmt(BPF_JMP | BPF_JA, u32::from(*offset) + 1),
                bpf_stmt(BPF_JMP | BPF_JA, u32::from(*offset) + 1),
            ];
            accumulator.push(helper_jumps);
            *offset = 1;
        }

        let condition = condition.into_bpf(*offset);
        // Safe to unwrap since we checked that offset + `CONDITION_MAX_LEN` does not overflow.
        *offset += u8::try_from(condition.len()).unwrap();
        accumulator.push(condition);
    }
}

impl From<SeccompRule> for BpfProgram {
    fn from(rule: SeccompRule) -> Self {
        // Each rule starts with 2 jump statements:
        // * The first jump enters the rule, attempting a match.
        // * The second one jumps out of the rule, into the next rule of the syscall or to the
        //   default action if none of the rules were matched.

        // Rule is built backwards, because SeccompConditions need to know the jump offset to the
        // next rule, when compiled to BPF.
        let mut accumulator = Vec::with_capacity(
            rule.conditions
                .len()
                // Realistically, this overflow should never happen.
                // If the nr of statements ever overflows `usize`, the rust vector allocation would
                // anyway fail.
                .checked_mul(CONDITION_MAX_LEN as usize)
                .unwrap(),
        );
        let mut offset = 1;

        // Conditions are translated into BPF statements and prepended to the rule.
        rule.conditions.into_iter().for_each(|condition| {
            SeccompRule::append_condition(condition, &mut accumulator, &mut offset)
        });

        // The two initial jump statements are prepended to the rule.
        accumulator.push(vec![
            bpf_stmt(BPF_JMP | BPF_JA, 1),
            bpf_stmt(BPF_JMP | BPF_JA, u32::from(offset) + 1),
        ]);

        // Finally, builds the translated rule by reversing and consuming the accumulator.
        let mut result = Vec::new();
        accumulator
            .into_iter()
            .rev()
            .for_each(|mut instructions| result.append(&mut instructions));

        result
    }
}

#[cfg(test)]
mod tests {
    use super::SeccompRule;
    use crate::backend::bpf::*;
    use crate::backend::{
        Error, SeccompCmpArgLen as ArgLen, SeccompCmpOp::*, SeccompCondition as Cond,
    };

    #[test]
    fn test_validate_rule() {
        assert_eq!(SeccompRule::new(vec![]).unwrap_err(), Error::EmptyRule);
    }

    // Checks that rule gets translated correctly into BPF statements.
    #[test]
    fn test_rule_bpf_output() {
        let rule = SeccompRule::new(vec![
            Cond::new(0, ArgLen::Dword, Eq, 1).unwrap(),
            Cond::new(2, ArgLen::Qword, MaskedEq(0b1010), 14).unwrap(),
        ])
        .unwrap();

        let (msb_offset, lsb_offset) = { (4, 0) };

        // Builds hardcoded BPF instructions.
        let instructions = vec![
            bpf_stmt(BPF_JMP | BPF_JA, 1),  // Start evaluating the rule.
            bpf_stmt(BPF_JMP | BPF_JA, 10), // Jump to the next rule.
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 32 + msb_offset),
            bpf_stmt(BPF_ALU | BPF_AND | BPF_K, 0),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 0, 0, 6),
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 32 + lsb_offset),
            bpf_stmt(BPF_ALU | BPF_AND | BPF_K, 0b1010),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 14 & 0b1010, 0, 3),
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 16 + lsb_offset),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 1, 0, 1),
        ];
        // In a filter, these instructions would follow:
        // RET match_action
        // OTHER RULES...
        // RET mismatch_action. (if the syscall number matched and then all rules fail to match)
        // RET default action. (if no syscall number matched)

        // Compares translated rule with hardcoded BPF instructions.
        let bpfprog: BpfProgram = rule.into();
        assert_eq!(bpfprog, instructions);
    }

    // Checks that rule with too many conditions gets translated correctly into BPF statements
    // using three helper jumps.
    #[test]
    fn test_rule_many_conditions_bpf_output() {
        // Builds rule.
        let mut conditions = Vec::with_capacity(43);
        for _ in 0..42 {
            conditions.push(Cond::new(0, ArgLen::Qword, MaskedEq(0), 0).unwrap());
        }
        conditions.push(Cond::new(0, ArgLen::Qword, Eq, 0).unwrap());
        let rule = SeccompRule::new(conditions).unwrap();

        let (msb_offset, lsb_offset) = { (4, 0) };

        // Builds hardcoded BPF instructions.
        let mut instructions = vec![
            bpf_stmt(BPF_JMP | BPF_JA, 1), // Start evaluating the rule.
            bpf_stmt(BPF_JMP | BPF_JA, 6), // Jump to the next rule. Actually to a helper jump.
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 16 + msb_offset),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 0, 0, 3),
            bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 16 + lsb_offset),
            bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 0, 0, 1),
            bpf_stmt(BPF_JMP | BPF_JA, 2),
            bpf_stmt(BPF_JMP | BPF_JA, 254),
            bpf_stmt(BPF_JMP | BPF_JA, 254),
        ];
        let mut offset = 253;
        for _ in 0..42 {
            offset -= 6;
            // Add the rest of the `MaskedEq` conditions.
            instructions.append(&mut vec![
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 16 + msb_offset),
                bpf_stmt(BPF_ALU | BPF_AND | BPF_K, 0),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 0, 0, offset + 3),
                bpf_stmt(BPF_LD | BPF_W | BPF_ABS, 16 + lsb_offset),
                bpf_stmt(BPF_ALU | BPF_AND | BPF_K, 0),
                bpf_jump(BPF_JMP | BPF_JEQ | BPF_K, 0, 0, offset),
            ]);
        }

        // Compares translated rule with hardcoded BPF instructions.
        let bpfprog: BpfProgram = rule.into();
        assert_eq!(bpfprog, instructions);
    }
}
