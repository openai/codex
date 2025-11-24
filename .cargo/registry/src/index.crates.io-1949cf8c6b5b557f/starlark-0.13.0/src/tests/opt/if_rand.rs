/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Permutations tests for if condition evaluation.

use std::cell::Cell;
use std::fmt;

use derive_more::Display;
use dupe::Dupe;
use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;

/// Count side effects. For example, in expression like:
///
/// ```text
/// false() and true()
/// ```
///
/// `true()` should not be evaluated. After evaluation, the counter should be 1.
#[derive(Debug, ProvidesStaticType, Default, PartialEq)]
struct CountCalls {
    calls: Cell<usize>,
}

#[starlark_module]
fn bool_fns(globals: &mut GlobalsBuilder) {
    /// Return `true` and record side effect.
    fn r#true(eval: &mut Evaluator) -> anyhow::Result<bool> {
        let calls = eval
            .extra
            .as_ref()
            .unwrap()
            .downcast_ref::<CountCalls>()
            .unwrap();
        calls.calls.set(calls.calls.get() + 1);
        Ok(true)
    }

    fn r#false(eval: &mut Evaluator) -> anyhow::Result<bool> {
        let calls = eval
            .extra
            .as_ref()
            .unwrap()
            .downcast_ref::<CountCalls>()
            .unwrap();
        calls.calls.set(calls.calls.get() + 1);
        Ok(false)
    }
}

#[derive(Display, Debug, Copy, Clone, Dupe)]
enum TestBinOp {
    #[display("and")]
    And,
    #[display("or")]
    Or,
}

impl TestBinOp {
    fn eval(self, x: bool, y: impl FnOnce() -> bool) -> bool {
        match self {
            TestBinOp::And => x && y(),
            TestBinOp::Or => x || y(),
        }
    }
}

#[derive(Clone, Debug)]
enum TestExpr {
    /// `True` or `False`.
    Const(bool),
    /// `true()` or `false()`.
    Count(bool),
    /// Binary operation.
    BinOp(TestBinOp, Box<(TestExpr, TestExpr)>),
    /// `not` operation.
    Not(Box<TestExpr>),
}

impl TestExpr {
    /// Evaluate the expression the same way Starlark would evaluate it.
    fn eval(&self, count: &CountCalls) -> bool {
        match self {
            TestExpr::Const(x) => *x,
            TestExpr::Count(x) => {
                // Record side effect.
                count.calls.set(count.calls.get() + 1);
                *x
            }
            TestExpr::BinOp(op, x_y) => {
                let (x, y) = &**x_y;
                op.eval(x.eval(count), || y.eval(count))
            }
            TestExpr::Not(x) => !x.eval(count),
        }
    }
}

impl Display for TestExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestExpr::Const(x) => match x {
                true => write!(f, "True"),
                false => write!(f, "False"),
            },
            TestExpr::Count(x) => match x {
                true => write!(f, "true()"),
                false => write!(f, "false()"),
            },
            TestExpr::BinOp(op, x_y) => {
                let (x, y) = &**x_y;
                write!(f, "({} {} {})", x, op, y)
            }
            TestExpr::Not(x) => write!(f, "(not {})", x),
        }
    }
}

/// Evaluate the program.
///
/// * Return the result which is expected to be `bool`.
/// * Count side effects.
fn eval_program(program: &str) -> (bool, CountCalls) {
    let module = Module::new();
    let ast = AstModule::parse("t.star", program.to_owned(), &Dialect::AllOptionsInternal).unwrap();

    let mut globals = GlobalsBuilder::standard();
    bool_fns(&mut globals);
    let globals = globals.build();

    let counts = CountCalls::default();
    let r = {
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&counts);
        let r = eval.eval_module(ast, &globals).unwrap();
        r.unpack_bool().unwrap()
    };
    (r, counts)
}

fn eval_if_else_with_starlark(expr: &TestExpr) -> (bool, CountCalls) {
    let program = format!(
        r#"
if {}:
    r = True
else:
    r = False
r
"#,
        expr
    );
    eval_program(&program)
}

fn eval_if_with_starlark(expr: &TestExpr) -> (bool, CountCalls) {
    let program = format!(
        r#"
r = False
if {}:
    r = True
r
"#,
        expr
    );
    eval_program(&program)
}

fn eval_expr_result(expr: &TestExpr) -> (bool, CountCalls) {
    eval_program(&expr.to_string())
}

fn eval_manually(expr: &TestExpr) -> (bool, CountCalls) {
    let counts = CountCalls::default();
    let r = expr.eval(&counts);
    (r, counts)
}

fn test_if_else(expr: &TestExpr) {
    let expected = eval_manually(expr);
    let actual = eval_if_else_with_starlark(expr);
    assert_eq!(expected, actual, "expression: {}", expr);
}

fn test_if(expr: &TestExpr) {
    let expected = eval_manually(expr);
    let actual = eval_if_with_starlark(expr);
    assert_eq!(expected, actual, "expression: {}", expr);
}

fn test_expr_result(expr: &TestExpr) {
    let expected = eval_manually(expr);
    let actual = eval_expr_result(expr);
    assert_eq!(expected, actual, "expression: {}", expr);
}

fn test_ifs(expr: &TestExpr) {
    test_if(expr);
    test_if_else(expr);
    // If condition expression compilation is different from compilation of the expression,
    // so we explicitly test both cases.
    test_expr_result(expr);
}

fn bool_values() -> [bool; 2] {
    [true, false]
}

fn basic_bool_exprs() -> impl Iterator<Item = TestExpr> {
    bool_values()
        .into_iter()
        .flat_map(|x| [TestExpr::Count(x), TestExpr::Const(x)])
}

#[test]
fn test_basic() {
    test_ifs(&TestExpr::Const(true));
    test_ifs(&TestExpr::Const(false));
    test_ifs(&TestExpr::Count(true));
    test_ifs(&TestExpr::Count(false));
    test_ifs(&TestExpr::Not(Box::new(TestExpr::Const(true))));
    test_ifs(&TestExpr::Not(Box::new(TestExpr::Const(false))));
    test_ifs(&TestExpr::Not(Box::new(TestExpr::Count(true))));
    test_ifs(&TestExpr::Not(Box::new(TestExpr::Count(false))));
}

#[test]
fn test_and() {
    for lhs in basic_bool_exprs() {
        for rhs in basic_bool_exprs() {
            test_ifs(&TestExpr::BinOp(
                TestBinOp::And,
                Box::new((lhs.clone(), rhs)),
            ));
        }
    }
}

#[test]
fn test_or() {
    for lhs in basic_bool_exprs() {
        for rhs in basic_bool_exprs() {
            test_ifs(&TestExpr::BinOp(
                TestBinOp::Or,
                Box::new((lhs.clone(), rhs)),
            ));
        }
    }
}

#[test]
fn test_and_or_not() {
    for lhs in basic_bool_exprs() {
        for rhs in basic_bool_exprs() {
            for negate_lhs in [false, true] {
                for negate_rhs in [false, true] {
                    for bin_op in [TestBinOp::And, TestBinOp::Or] {
                        let lhs = if negate_lhs {
                            TestExpr::Not(Box::new(lhs.clone()))
                        } else {
                            lhs.clone()
                        };
                        let rhs = if negate_rhs {
                            TestExpr::Not(Box::new(rhs.clone()))
                        } else {
                            rhs.clone()
                        };
                        test_ifs(&TestExpr::BinOp(bin_op, Box::new((lhs, rhs))));
                    }
                }
            }
        }
    }
}

const RANDOM_ITERATIONS: usize = 100;

fn max_depth_for_iter(i: usize) -> usize {
    if i < 5 {
        0
    } else if i < RANDOM_ITERATIONS / 50 {
        1
    } else if i < RANDOM_ITERATIONS / 25 {
        2
    } else if i < RANDOM_ITERATIONS / 10 {
        3
    } else if i < RANDOM_ITERATIONS / 3 {
        4
    } else if i < RANDOM_ITERATIONS / 2 {
        5
    } else {
        20
    }
}

fn random_expr(rng: &mut SmallRng, max_depth: usize) -> TestExpr {
    fn random_simple_expr(rng: &mut SmallRng) -> TestExpr {
        match rng.gen_range(0..4) {
            0 => TestExpr::Const(true),
            1 => TestExpr::Const(false),
            2 => TestExpr::Count(true),
            3 => TestExpr::Count(false),
            _ => unreachable!(),
        }
    }

    if max_depth == 0 {
        random_simple_expr(rng)
    } else {
        match rng.gen_range(0..4) {
            0 => random_simple_expr(rng),
            1 => TestExpr::Not(Box::new(random_expr(rng, max_depth - 1))),
            2 => TestExpr::BinOp(
                TestBinOp::And,
                Box::new((
                    random_expr(rng, max_depth - 1),
                    random_expr(rng, max_depth - 1),
                )),
            ),
            3 => TestExpr::BinOp(
                TestBinOp::Or,
                Box::new((
                    random_expr(rng, max_depth - 1),
                    random_expr(rng, max_depth - 1),
                )),
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_if_random() {
    let mut rng = SmallRng::seed_from_u64(17);
    for i in 0..RANDOM_ITERATIONS {
        let max_depth = max_depth_for_iter(i);
        let expr = random_expr(&mut rng, max_depth);
        test_if(&expr);
    }
}

#[test]
fn test_if_else_random() {
    let mut rng = SmallRng::seed_from_u64(17);
    for i in 0..RANDOM_ITERATIONS {
        let max_depth = max_depth_for_iter(i);
        let expr = random_expr(&mut rng, max_depth);
        test_if_else(&expr);
    }
}

#[test]
fn test_expr_random() {
    let mut rng = SmallRng::seed_from_u64(17);
    for i in 0..RANDOM_ITERATIONS {
        let max_depth = max_depth_for_iter(i);
        let expr = random_expr(&mut rng, max_depth);
        test_expr_result(&expr);
    }
}
