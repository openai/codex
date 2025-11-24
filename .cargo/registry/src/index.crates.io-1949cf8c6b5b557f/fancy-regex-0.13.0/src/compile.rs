// Copyright 2016 The Fancy Regex Authors.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Compilation of regexes to VM.

use alloc::string::String;
use alloc::vec::Vec;
use core::usize;
use regex_automata::meta::Regex as RaRegex;
use regex_automata::meta::{Builder as RaBuilder, Config as RaConfig};
#[cfg(all(test, feature = "std"))]
use std::{collections::BTreeMap, sync::RwLock};

use crate::analyze::Info;
use crate::vm::{Insn, Prog};
use crate::LookAround::*;
use crate::{CompileError, Error, Expr, LookAround, RegexOptions, Result};

// I'm thinking it probably doesn't make a lot of sense having this split
// out from Compiler.
struct VMBuilder {
    prog: Vec<Insn>,
    n_saves: usize,
}

impl VMBuilder {
    fn new(max_group: usize) -> VMBuilder {
        VMBuilder {
            prog: Vec::new(),
            n_saves: max_group * 2,
        }
    }

    fn build(self) -> Prog {
        Prog::new(self.prog, self.n_saves)
    }

    fn newsave(&mut self) -> usize {
        let result = self.n_saves;
        self.n_saves += 1;
        result
    }

    fn pc(&self) -> usize {
        self.prog.len()
    }

    // would "emit" be a better name?
    fn add(&mut self, insn: Insn) {
        self.prog.push(insn);
    }

    fn set_jmp_target(&mut self, jmp_pc: usize, target: usize) {
        match self.prog[jmp_pc] {
            Insn::Jmp(ref mut next) => *next = target,
            _ => panic!("mutating instruction other than Jmp"),
        }
    }

    fn set_split_target(&mut self, split_pc: usize, target: usize, second: bool) {
        match self.prog[split_pc] {
            Insn::Split(_, ref mut y) if second => *y = target,
            Insn::Split(ref mut x, _) => *x = target,
            _ => panic!("mutating instruction other than Split"),
        }
    }

    fn set_repeat_target(&mut self, repeat_pc: usize, target: usize) {
        match self.prog[repeat_pc] {
            Insn::RepeatGr { ref mut next, .. }
            | Insn::RepeatNg { ref mut next, .. }
            | Insn::RepeatEpsilonGr { ref mut next, .. }
            | Insn::RepeatEpsilonNg { ref mut next, .. } => *next = target,
            _ => panic!("mutating instruction other than Repeat"),
        }
    }
}

struct Compiler {
    b: VMBuilder,
    options: RegexOptions,
}

impl Compiler {
    fn new(max_group: usize) -> Compiler {
        Compiler {
            b: VMBuilder::new(max_group),
            options: Default::default(),
        }
    }

    fn visit(&mut self, info: &Info<'_>, hard: bool) -> Result<()> {
        if !hard && !info.hard {
            // easy case, delegate entire subexpr
            return self.compile_delegate(info);
        }
        match *info.expr {
            Expr::Empty => (),
            Expr::Literal { ref val, casei } => {
                if !casei {
                    self.b.add(Insn::Lit(val.clone()));
                } else {
                    self.compile_delegate(info)?;
                }
            }
            Expr::Any { newline: true } => {
                self.b.add(Insn::Any);
            }
            Expr::Any { newline: false } => {
                self.b.add(Insn::AnyNoNL);
            }
            Expr::Concat(_) => {
                self.compile_concat(info, hard)?;
            }
            Expr::Alt(_) => {
                let count = info.children.len();
                self.compile_alt(count, |compiler, i| compiler.visit(&info.children[i], hard))?;
            }
            Expr::Group(_) => {
                let group = info.start_group;
                self.b.add(Insn::Save(group * 2));
                self.visit(&info.children[0], hard)?;
                self.b.add(Insn::Save(group * 2 + 1));
            }
            Expr::Repeat { lo, hi, greedy, .. } => {
                self.compile_repeat(info, lo, hi, greedy, hard)?;
            }
            Expr::LookAround(_, la) => {
                self.compile_lookaround(info, la)?;
            }
            Expr::Backref(group) => {
                self.b.add(Insn::Backref(group * 2));
            }
            Expr::BackrefExistsCondition(group) => {
                self.b.add(Insn::BackrefExistsCondition(group));
            }
            Expr::AtomicGroup(_) => {
                // TODO optimization: atomic insns are not needed if the
                // child doesn't do any backtracking.
                self.b.add(Insn::BeginAtomic);
                self.visit(&info.children[0], false)?;
                self.b.add(Insn::EndAtomic);
            }
            Expr::Delegate { .. } => {
                // TODO: might want to have more specialized impls
                self.compile_delegate(info)?;
            }
            Expr::Assertion(assertion) => {
                self.b.add(Insn::Assertion(assertion));
            }
            Expr::KeepOut => {
                self.b.add(Insn::Save(0));
            }
            Expr::ContinueFromPreviousMatchEnd => {
                self.b.add(Insn::ContinueFromPreviousMatchEnd);
            }
            Expr::Conditional { .. } => {
                self.compile_conditional(|compiler, i| compiler.visit(&info.children[i], hard))?;
            }
        }
        Ok(())
    }

    fn compile_alt<F>(&mut self, count: usize, mut handle_alternative: F) -> Result<()>
    where
        F: FnMut(&mut Compiler, usize) -> Result<()>,
    {
        let mut jmps = Vec::new();
        let mut last_pc = usize::MAX;
        for i in 0..count {
            let has_next = i != count - 1;
            let pc = self.b.pc();
            if has_next {
                self.b.add(Insn::Split(pc + 1, usize::MAX));
            }
            if last_pc != usize::MAX {
                self.b.set_split_target(last_pc, pc, true);
            }
            last_pc = pc;

            handle_alternative(self, i)?;

            if has_next {
                // All except the last branch need to jump over instructions of
                // other branches. The last branch can just continue to the next
                // instruction.
                let pc = self.b.pc();
                jmps.push(pc);
                self.b.add(Insn::Jmp(0));
            }
        }
        let next_pc = self.b.pc();
        for jmp_pc in jmps {
            self.b.set_jmp_target(jmp_pc, next_pc);
        }
        Ok(())
    }

    fn compile_conditional<F>(&mut self, mut handle_child: F) -> Result<()>
    where
        F: FnMut(&mut Compiler, usize) -> Result<()>,
    {
        // here we use atomic group functionality to be able to remove the program counter
        // relating to the split instruction's second position if the conditional succeeds
        // This is to ensure that if the condition succeeds, but the "true" branch from the
        // conditional fails, that it wouldn't jump to the "false" branch.
        self.b.add(Insn::BeginAtomic);

        let split_pc = self.b.pc();
        // add the split instruction - we will update it's second pc later
        self.b.add(Insn::Split(split_pc + 1, usize::MAX));

        // add the conditional expression
        handle_child(self, 0)?;

        // mark it as successful to remove the state we added as a split earlier
        self.b.add(Insn::EndAtomic);

        // add the truth branch
        handle_child(self, 1)?;
        // add an instruction to jump over the false branch - we will update the jump target later
        let jump_over_false_pc = self.b.pc();
        self.b.add(Insn::Jmp(0));

        // add the false branch, update the split target
        self.b.set_split_target(split_pc, self.b.pc(), true);
        handle_child(self, 2)?;

        // update the jump target for jumping over the false branch
        self.b.set_jmp_target(jump_over_false_pc, self.b.pc());

        Ok(())
    }

    fn compile_concat(&mut self, info: &Info<'_>, hard: bool) -> Result<()> {
        // First: determine a prefix which is constant size and not hard.
        let prefix_end = info
            .children
            .iter()
            .take_while(|c| c.const_size && !c.hard)
            .count();

        // If incoming difficulty is not hard, the suffix after the last
        // hard child can be done with NFA.
        let suffix_len = if !hard {
            info.children[prefix_end..]
                .iter()
                .rev()
                .take_while(|c| !c.hard)
                .count()
        } else {
            // Even for hard, we can delegate a const-sized suffix
            info.children[prefix_end..]
                .iter()
                .rev()
                .take_while(|c| c.const_size && !c.hard)
                .count()
        };
        let suffix_begin = info.children.len() - suffix_len;

        self.compile_delegates(&info.children[..prefix_end])?;

        for child in info.children[prefix_end..suffix_begin].iter() {
            self.visit(child, true)?;
        }

        self.compile_delegates(&info.children[suffix_begin..])
    }

    fn compile_repeat(
        &mut self,
        info: &Info<'_>,
        lo: usize,
        hi: usize,
        greedy: bool,
        hard: bool,
    ) -> Result<()> {
        let child = &info.children[0];
        if lo == 0 && hi == 1 {
            // e?
            let pc = self.b.pc();
            self.b.add(Insn::Split(pc + 1, pc + 1));
            // TODO: do we want to do an epsilon check here? If we do
            // it here and in Alt, we might be able to make a good
            // bound on stack depth
            self.visit(child, hard)?;
            let next_pc = self.b.pc();
            self.b.set_split_target(pc, next_pc, greedy);
            return Ok(());
        }
        let hard = hard | info.hard;
        if hi == usize::MAX && child.min_size == 0 {
            // Use RepeatEpsilon instructions to prevent empty repeat
            let repeat = self.b.newsave();
            let check = self.b.newsave();
            self.b.add(Insn::Save0(repeat));
            let pc = self.b.pc();
            if greedy {
                self.b.add(Insn::RepeatEpsilonGr {
                    lo,
                    next: usize::MAX,
                    repeat,
                    check,
                });
            } else {
                self.b.add(Insn::RepeatEpsilonNg {
                    lo,
                    next: usize::MAX,
                    repeat,
                    check,
                });
            }
            self.visit(child, hard)?;
            self.b.add(Insn::Jmp(pc));
            let next_pc = self.b.pc();
            self.b.set_repeat_target(pc, next_pc);
        } else if lo == 0 && hi == usize::MAX {
            // e*
            let pc = self.b.pc();
            self.b.add(Insn::Split(pc + 1, pc + 1));
            self.visit(child, hard)?;
            self.b.add(Insn::Jmp(pc));
            let next_pc = self.b.pc();
            self.b.set_split_target(pc, next_pc, greedy);
        } else if lo == 1 && hi == usize::MAX {
            // e+
            let pc = self.b.pc();
            self.visit(child, hard)?;
            let next = self.b.pc() + 1;
            let (x, y) = if greedy { (pc, next) } else { (next, pc) };
            self.b.add(Insn::Split(x, y));
        } else {
            let repeat = self.b.newsave();
            self.b.add(Insn::Save0(repeat));
            let pc = self.b.pc();
            if greedy {
                self.b.add(Insn::RepeatGr {
                    lo,
                    hi,
                    next: usize::MAX,
                    repeat,
                });
            } else {
                self.b.add(Insn::RepeatNg {
                    lo,
                    hi,
                    next: usize::MAX,
                    repeat,
                });
            }
            self.visit(child, hard)?;
            self.b.add(Insn::Jmp(pc));
            let next_pc = self.b.pc();
            self.b.set_repeat_target(pc, next_pc);
        }
        Ok(())
    }

    fn compile_lookaround(&mut self, info: &Info<'_>, la: LookAround) -> Result<()> {
        let inner = &info.children[0];
        match la {
            LookBehind => {
                if let Info {
                    const_size: false,
                    expr: &Expr::Alt(_),
                    ..
                } = inner
                {
                    // Make const size by transforming `(?<=a|bb)` to `(?<=a)|(?<=bb)`
                    let alternatives = &inner.children;
                    self.compile_alt(alternatives.len(), |compiler, i| {
                        let alternative = &alternatives[i];
                        compiler.compile_positive_lookaround(alternative, la)
                    })
                } else {
                    self.compile_positive_lookaround(inner, la)
                }
            }
            LookBehindNeg => {
                if let Info {
                    const_size: false,
                    expr: &Expr::Alt(_),
                    ..
                } = inner
                {
                    // Make const size by transforming `(?<!a|bb)` to `(?<!a)(?<!bb)`
                    let alternatives = &inner.children;
                    for alternative in alternatives {
                        self.compile_negative_lookaround(alternative, la)?;
                    }
                    Ok(())
                } else {
                    self.compile_negative_lookaround(inner, la)
                }
            }
            LookAhead => self.compile_positive_lookaround(inner, la),
            LookAheadNeg => self.compile_negative_lookaround(inner, la),
        }
    }

    fn compile_positive_lookaround(&mut self, inner: &Info<'_>, la: LookAround) -> Result<()> {
        let save = self.b.newsave();
        self.b.add(Insn::Save(save));
        self.compile_lookaround_inner(inner, la)?;
        self.b.add(Insn::Restore(save));
        Ok(())
    }

    fn compile_negative_lookaround(&mut self, inner: &Info<'_>, la: LookAround) -> Result<()> {
        let pc = self.b.pc();
        self.b.add(Insn::Split(pc + 1, usize::MAX));
        self.compile_lookaround_inner(inner, la)?;
        self.b.add(Insn::FailNegativeLookAround);
        let next_pc = self.b.pc();
        self.b.set_split_target(pc, next_pc, true);
        Ok(())
    }

    fn compile_lookaround_inner(&mut self, inner: &Info<'_>, la: LookAround) -> Result<()> {
        if la == LookBehind || la == LookBehindNeg {
            if !inner.const_size {
                return Err(Error::CompileError(CompileError::LookBehindNotConst));
            }
            self.b.add(Insn::GoBack(inner.min_size));
        }
        self.visit(inner, false)
    }

    fn compile_delegates(&mut self, infos: &[Info<'_>]) -> Result<()> {
        if infos.is_empty() {
            return Ok(());
        }
        // TODO: might want to do something similar for case insensitive literals
        // (have is_literal return an additional bool for casei)
        if infos.iter().all(|e| e.is_literal()) {
            let mut val = String::new();
            for info in infos {
                info.push_literal(&mut val);
            }
            self.b.add(Insn::Lit(val));
            return Ok(());
        }

        let mut delegate_builder = DelegateBuilder::new();
        for info in infos {
            delegate_builder.push(info);
        }
        let delegate = delegate_builder.build(&self.options)?;

        self.b.add(delegate);
        Ok(())
    }

    fn compile_delegate(&mut self, info: &Info) -> Result<()> {
        let insn = if info.is_literal() {
            let mut val = String::new();
            info.push_literal(&mut val);
            Insn::Lit(val)
        } else {
            DelegateBuilder::new().push(info).build(&self.options)?
        };
        self.b.add(insn);
        Ok(())
    }
}

// Unlike Regex in `regex`, `regex-automata` does not store the pattern string,
// and we cannot retrieve the pattern string using `as_str`.
// Unfortunately we need to get the pattern string in our tests,
// so we just store it in a global map.
#[cfg(all(test, feature = "std"))]
static PATTERN_MAPPING: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());

pub(crate) fn compile_inner(inner_re: &str, options: &RegexOptions) -> Result<RaRegex> {
    let mut config = RaConfig::new();
    if let Some(size_limit) = options.delegate_size_limit {
        config = config.nfa_size_limit(Some(size_limit));
    }
    if let Some(dfa_size_limit) = options.delegate_dfa_size_limit {
        config = config.dfa_size_limit(Some(dfa_size_limit));
    }

    let re = RaBuilder::new()
        .configure(config)
        .build(inner_re)
        .map_err(CompileError::InnerError)
        .map_err(Error::CompileError)?;

    #[cfg(all(test, feature = "std"))]
    PATTERN_MAPPING
        .write()
        .unwrap()
        .insert(format!("{:?}", re), inner_re.to_owned());

    Ok(re)
}

/// Compile the analyzed expressions into a program.
pub fn compile(info: &Info<'_>) -> Result<Prog> {
    let mut c = Compiler::new(info.end_group);
    c.visit(info, false)?;
    c.b.add(Insn::End);
    Ok(c.b.build())
}

struct DelegateBuilder {
    re: String,
    min_size: usize,
    const_size: bool,
    start_group: Option<usize>,
    end_group: usize,
}

impl DelegateBuilder {
    fn new() -> Self {
        Self {
            re: String::new(),
            min_size: 0,
            const_size: true,
            start_group: None,
            end_group: 0,
        }
    }

    fn push(&mut self, info: &Info<'_>) -> &mut DelegateBuilder {
        // TODO: might want to detect case of a group with no captures
        //  inside, so we can run find() instead of captures()

        self.min_size += info.min_size;
        self.const_size &= info.const_size;
        if self.start_group.is_none() {
            self.start_group = Some(info.start_group);
        }
        self.end_group = info.end_group;

        // Add expression. The precedence argument has to be 1 here to
        // ensure correct grouping in these cases:
        //
        // If we have multiple expressions, we are building a concat.
        // Without grouping, we'd turn ["a", "b|c"] into "^ab|c". But we
        // want "^a(?:b|c)".
        //
        // Even with a single expression, because we add `^` at the
        // beginning, we need a group. Otherwise `["a|b"]` would be turned
        // into `"^a|b"` instead of `"^(?:a|b)"`.
        info.expr.to_str(&mut self.re, 1);
        self
    }

    fn build(&self, options: &RegexOptions) -> Result<Insn> {
        let start_group = self.start_group.expect("Expected at least one expression");
        let end_group = self.end_group;

        let compiled = compile_inner(&self.re, options)?;

        Ok(Insn::Delegate {
            inner: compiled,
            start_group,
            end_group,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::analyze;
    use crate::parse::ExprTree;
    use crate::vm::Insn::*;
    use alloc::vec;
    use bit_set::BitSet;
    use matches::assert_matches;

    #[test]
    fn jumps_for_alternation() {
        let tree = ExprTree {
            expr: Expr::Alt(vec![
                Expr::Literal {
                    val: "a".into(),
                    casei: false,
                },
                Expr::Literal {
                    val: "b".into(),
                    casei: false,
                },
                Expr::Literal {
                    val: "c".into(),
                    casei: false,
                },
            ]),
            backrefs: BitSet::new(),
            named_groups: Default::default(),
        };
        let info = analyze(&tree).unwrap();

        let mut c = Compiler::new(0);
        // Force "hard" so that compiler doesn't just delegate
        c.visit(&info, true).unwrap();
        c.b.add(Insn::End);

        let prog = c.b.prog;

        assert_eq!(prog.len(), 8, "prog: {:?}", prog);
        assert_matches!(prog[0], Split(1, 3));
        assert_matches!(prog[1], Lit(ref l) if l == "a");
        assert_matches!(prog[2], Jmp(7));
        assert_matches!(prog[3], Split(4, 6));
        assert_matches!(prog[4], Lit(ref l) if l == "b");
        assert_matches!(prog[5], Jmp(7));
        assert_matches!(prog[6], Lit(ref l) if l == "c");
        assert_matches!(prog[7], End);
    }

    #[cfg_attr(not(feature = "std"), ignore = "this test need std")]
    #[test]
    fn look_around_pattern_can_be_delegated() {
        let prog = compile_prog("(?=ab*)c");

        assert_eq!(prog.len(), 5, "prog: {:?}", prog);
        assert_matches!(prog[0], Save(0));
        assert_delegate(&prog[1], "ab*");
        assert_matches!(prog[2], Restore(0));
        assert_matches!(prog[3], Lit(ref l) if l == "c");
        assert_matches!(prog[4], End);
    }

    #[cfg_attr(not(feature = "std"), ignore = "this test need std")]
    #[test]
    fn easy_concat_can_delegate_end() {
        let prog = compile_prog("(?!x)(?:a|ab)x*");

        assert_eq!(prog.len(), 5, "prog: {:?}", prog);
        assert_matches!(prog[0], Split(1, 3));
        assert_matches!(prog[1], Lit(ref l) if l == "x");
        assert_matches!(prog[2], FailNegativeLookAround);
        assert_delegate(&prog[3], "(?:a|ab)x*");
        assert_matches!(prog[4], End);
    }

    #[cfg_attr(not(feature = "std"), ignore = "this test need std")]
    #[test]
    fn hard_concat_can_delegate_const_size_end() {
        let prog = compile_prog("(?:(?!x)(?:a|b)c)x*");

        assert_eq!(prog.len(), 6, "prog: {:?}", prog);
        assert_matches!(prog[0], Split(1, 3));
        assert_matches!(prog[1], Lit(ref l) if l == "x");
        assert_matches!(prog[2], FailNegativeLookAround);
        assert_delegate(&prog[3], "(?:a|b)c");
        assert_delegate(&prog[4], "x*");
        assert_matches!(prog[5], End);
    }

    #[cfg_attr(not(feature = "std"), ignore = "this test need std")]
    #[test]
    fn hard_concat_can_not_delegate_variable_end() {
        let prog = compile_prog("(?:(?!x)(?:a|ab))x*");

        assert_eq!(prog.len(), 9, "prog: {:?}", prog);
        assert_matches!(prog[0], Split(1, 3));
        assert_matches!(prog[1], Lit(ref l) if l == "x");
        assert_matches!(prog[2], FailNegativeLookAround);
        assert_matches!(prog[3], Split(4, 6));
        assert_matches!(prog[4], Lit(ref l) if l == "a");
        assert_matches!(prog[5], Jmp(7));
        assert_matches!(prog[6], Lit(ref l) if l == "ab");
        assert_delegate(&prog[7], "x*");
        assert_matches!(prog[8], End);
    }

    #[test]
    fn conditional_expression_can_be_compiled() {
        let prog = compile_prog(r"(?(ab)c|d)");

        assert_eq!(prog.len(), 8, "prog: {:?}", prog);

        assert_matches!(prog[0], BeginAtomic);
        assert_matches!(prog[1], Split(2, 6));
        assert_matches!(prog[2], Lit(ref l) if l == "ab");
        assert_matches!(prog[3], EndAtomic);
        assert_matches!(prog[4], Lit(ref l) if l == "c");
        assert_matches!(prog[5], Jmp(7));
        assert_matches!(prog[6], Lit(ref l) if l == "d");
        assert_matches!(prog[7], End);
    }

    fn compile_prog(re: &str) -> Vec<Insn> {
        let tree = Expr::parse_tree(re).unwrap();
        let info = analyze(&tree).unwrap();
        let prog = compile(&info).unwrap();
        prog.body
    }

    #[cfg(feature = "std")]
    fn assert_delegate(insn: &Insn, re: &str) {
        match insn {
            Insn::Delegate { inner, .. } => {
                assert_eq!(
                    PATTERN_MAPPING
                        .read()
                        .unwrap()
                        .get(&alloc::format!("{:?}", inner))
                        .unwrap(),
                    re
                );
            }
            _ => {
                panic!("Expected Insn::Delegate but was {:#?}", insn);
            }
        }
    }

    #[cfg(not(feature = "std"))]
    fn assert_delegate(_: &Insn, _: &str) {}
}
