/*
 * Copyright 2019 The Starlark in Rust Authors.
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

use starlark_syntax::syntax::ast::AstExpr;
use starlark_syntax::syntax::ast::AstLiteral;
use starlark_syntax::syntax::ast::AstStmt;
use starlark_syntax::syntax::ast::AstTypeExpr;
use starlark_syntax::syntax::ast::DefP;
use starlark_syntax::syntax::ast::Expr;
use starlark_syntax::syntax::ast::ForP;
use starlark_syntax::syntax::ast::Stmt;
use starlark_syntax::syntax::module::AstModuleFields;
use thiserror::Error;

use crate::analysis::types::LintT;
use crate::analysis::types::LintWarning;
use crate::analysis::EvalSeverity;
use crate::codemap::CodeMap;
use crate::codemap::ResolvedFileSpan;
use crate::codemap::Span;
use crate::codemap::Spanned;
use crate::syntax::AstModule;

#[derive(Error, Debug)]
pub(crate) enum FlowIssue {
    #[error("`return` lacks expression, but function `{0}` at {1} seems to want one due to {2}")]
    MissingReturnExpression(String, ResolvedFileSpan, ResolvedFileSpan),
    #[error("No `return` at the end, but function `{0}` seems to want one due to {1}")]
    MissingReturn(String, ResolvedFileSpan),
    #[error("Unreachable statement `{0}`")]
    Unreachable(String),
    #[error("Redundant `return` at the end of a function")]
    RedundantReturn,
    #[error("Redundant `continue` at the end of a loop")]
    RedundantContinue,
    #[error("A `load` statement not at the top of the file")]
    MisplacedLoad,
    #[error("Statement at has no effect")]
    NoEffect,
}

impl LintWarning for FlowIssue {
    fn severity(&self) -> EvalSeverity {
        match self {
            // Sometimes people add these to make flow clearer
            FlowIssue::RedundantContinue | FlowIssue::RedundantReturn => EvalSeverity::Disabled,
            _ => EvalSeverity::Warning,
        }
    }

    fn short_name(&self) -> &'static str {
        match self {
            FlowIssue::MissingReturnExpression(..) => "missing-return-expression",
            FlowIssue::MissingReturn(..) => "missing-return",
            FlowIssue::Unreachable(..) => "unreachable",
            FlowIssue::RedundantReturn => "redundant-return",
            FlowIssue::RedundantContinue => "redundant-continue",
            FlowIssue::MisplacedLoad => "misplaced-load",
            FlowIssue::NoEffect => "no-effect",
        }
    }
}

fn returns(x: &AstStmt) -> Vec<(Span, Option<&AstExpr>)> {
    fn f<'a>(x: &'a AstStmt, res: &mut Vec<(Span, Option<&'a AstExpr>)>) {
        match &**x {
            Stmt::Return(ret) => res.push((x.span, ret.as_ref())),
            Stmt::Def(..) => {} // Do not descend
            _ => x.visit_stmt(|x| f(x, res)),
        }
    }

    let mut res = Vec::new();
    f(x, &mut res);
    res
}

// fail is kind of like a return with error
fn is_fail(x: &AstExpr) -> bool {
    match &**x {
        Expr::Call(x, _) => match &***x {
            Expr::Identifier(name) => name.node.ident == "fail",
            _ => false,
        },
        _ => false,
    }
}

fn has_effect(x: &AstExpr) -> bool {
    match &**x {
        Expr::Literal(x) => {
            // String literals have the "effect" of providing documentation
            matches!(x, AstLiteral::String(_))
        }
        Expr::Lambda(_) => false,
        Expr::If(_) | Expr::Tuple(_) | Expr::List(_) | Expr::Dict(_) => {
            let mut res = false;
            x.visit_expr(|x| res = res || has_effect(x));
            res
        }
        _ => true,
    }
}

fn final_return(x: &AstStmt) -> bool {
    match &**x {
        Stmt::Return(_) => true,
        Stmt::Expression(x) if is_fail(x) => true,
        Stmt::Statements(xs) => match xs.last() {
            None => false,
            Some(x) => final_return(x),
        },
        Stmt::IfElse(_, x_y) => {
            let (x, y) = &**x_y;
            final_return(x) && final_return(y)
        }
        _ => false,
    }
}

fn require_return_expression(ret_type: &Option<Box<AstTypeExpr>>) -> Option<Span> {
    match ret_type {
        None => None,
        Some(x) => match &x.node.expr.node {
            Expr::Identifier(x) if x.node.ident == "None" => None,
            _ => Some(x.span),
        },
    }
}

fn check_stmt(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) {
    match &**x {
        Stmt::Def(DefP {
            name,
            params: _,
            return_type,
            body,
            payload: _,
        }) => {
            let rets = returns(body);

            // Do I require my return statements to have an expression
            let require_expression = require_return_expression(return_type)
                .or_else(|| rets.iter().find(|x| x.1.is_some()).map(|x| x.0));
            if let Some(reason) = require_expression {
                if !final_return(body) {
                    res.push(LintT::new(
                        codemap,
                        x.span,
                        FlowIssue::MissingReturn(
                            // Statements often end with \n, so remove that to fit nicely
                            name.node.ident.trim_end().to_owned(),
                            codemap.file_span(reason).resolve(),
                        ),
                    ));
                }
                for (span, ret) in rets {
                    if ret.is_none() {
                        res.push(LintT::new(
                            codemap,
                            span,
                            FlowIssue::MissingReturnExpression(
                                name.ident.clone(),
                                codemap.file_span(x.span).resolve(),
                                codemap.file_span(reason).resolve(),
                            ),
                        ))
                    }
                }
            }
        }
        _ => {}
    }
}

fn stmt(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) {
    check_stmt(codemap, x, res);
    x.visit_stmt(|x| stmt(codemap, x, res));
}

// Returns true if the code aborts this sequence early, due to return, fail, break or continue
fn reachable(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) -> bool {
    match &**x {
        Stmt::Break | Stmt::Continue | Stmt::Return(_) => true,
        Stmt::Expression(x) => is_fail(x),
        Stmt::Statements(xs) => {
            let mut i = xs.iter();
            while let Some(x) = i.next() {
                let aborts = reachable(codemap, x, res);
                if aborts {
                    if let Some(nxt) = i.next() {
                        res.push(LintT::new(
                            codemap,
                            nxt.span,
                            FlowIssue::Unreachable(nxt.node.to_string().trim().to_owned()),
                        ))
                    }
                    // All the remaining statements are totally unreachable, but we declared that once
                    // so don't even bother looking at them
                    return aborts;
                }
            }
            false
        }
        Stmt::IfElse(_, x_y) => {
            let (x, y) = &**x_y;
            let abort1 = reachable(codemap, x, res);
            let abort2 = reachable(codemap, y, res);
            abort1 && abort2
        }
        // For all remaining constructs, visit their children to accumulate errors,
        // but even if they are present with returns, you don't guarantee the code with inner returns
        // gets executed.
        _ => {
            x.visit_stmt(|x| {
                reachable(codemap, x, res);
            });
            false
        }
    }
}

// If you have a definition which ends with return, or a loop which ends with continue
// that is a useless statement that just
fn redundant(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) {
    fn check(is_loop: bool, codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) {
        match &**x {
            Stmt::Continue if is_loop => {
                res.push(LintT::new(codemap, x.span, FlowIssue::RedundantContinue))
            }
            Stmt::Return(None) if !is_loop => {
                res.push(LintT::new(codemap, x.span, FlowIssue::RedundantReturn))
            }
            Stmt::Statements(xs) if !xs.is_empty() => {
                check(is_loop, codemap, xs.last().unwrap(), res)
            }
            Stmt::If(_, x) => check(is_loop, codemap, x, res),
            Stmt::IfElse(_, x_y) => {
                let (x, y) = &**x_y;
                check(is_loop, codemap, x, res);
                check(is_loop, codemap, y, res);
            }
            _ => {}
        }
    }

    fn f(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) {
        match &**x {
            Stmt::For(ForP { body, .. }) => check(true, codemap, body, res),
            Stmt::Def(DefP { body, .. }) => check(false, codemap, body, res),
            _ => {}
        }
        // We always want to look inside everything for other types of violation
        x.visit_stmt(|x| f(codemap, x, res))
    }

    x.visit_stmt(|x| f(codemap, x, res));
}

fn misplaced_load(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) {
    // accumulate all statements at the top-level
    fn top_statements<'a>(x: &'a AstStmt, stmts: &mut Vec<&'a AstStmt>) {
        match &**x {
            Stmt::Statements(xs) => {
                for x in xs {
                    top_statements(x, stmts);
                }
            }
            _ => stmts.push(x),
        }
    }

    let mut stmts = Vec::new();
    top_statements(x, &mut stmts);

    // We allow loads or documentation strings, but after that, no loads
    let mut allow_loads = true;
    for x in stmts {
        match &**x {
            Stmt::Load(..) => {
                if !allow_loads {
                    res.push(LintT::new(codemap, x.span, FlowIssue::MisplacedLoad))
                }
            }
            Stmt::Expression(Spanned {
                node: Expr::Literal(AstLiteral::String(_)),
                ..
            }) => {
                // Still allow loads after a literal string (probably documentation)
            }
            _ => allow_loads = false,
        }
    }
}

fn no_effect(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<FlowIssue>>) {
    match &**x {
        Stmt::Expression(x) if !has_effect(x) => {
            res.push(LintT::new(codemap, x.span, FlowIssue::NoEffect))
        }
        _ => x.visit_stmt(|x| no_effect(codemap, x, res)),
    }
}

pub(crate) fn lint(module: &AstModule) -> Vec<LintT<FlowIssue>> {
    let mut res = Vec::new();
    stmt(module.codemap(), module.statement(), &mut res);
    reachable(module.codemap(), module.statement(), &mut res);
    redundant(module.codemap(), module.statement(), &mut res);
    misplaced_load(module.codemap(), module.statement(), &mut res);
    no_effect(module.codemap(), module.statement(), &mut res);
    res
}

#[cfg(test)]
mod tests {
    use starlark_syntax::slice_vec_ext::SliceExt;

    use super::*;
    use crate::syntax::Dialect;

    fn module(x: &str) -> AstModule {
        AstModule::parse("X", x.to_owned(), &Dialect::AllOptionsInternal).unwrap()
    }

    impl FlowIssue {
        fn about(&self) -> &String {
            match self {
                FlowIssue::MissingReturnExpression(x, _, _) => x,
                FlowIssue::MissingReturn(x, _) => x,
                FlowIssue::Unreachable(x) => x,
                _ => panic!("Should not be used on such issues in test code"),
            }
        }
    }

    #[test]
    fn test_lint_returns() {
        let m = module(
            r#"
def no1() -> str:
    pass
def no2():
    if x:
        return 1
def no3():
    if x:
        return
    return 1
def ok():
    def no4() -> int:
        no4()
    pass
def yes1():
    pass
def yes2() -> str:
    yes1()
    if x:
        return "x"
    else:
        return "y"
def yes3():
    if x:
        return
    pass
def yes4() -> str:
    fail("die")
"#,
        );
        let mut res = Vec::new();
        stmt(m.codemap(), m.statement(), &mut res);
        assert_eq!(
            res.map(|x| x.problem.about()),
            &["no1", "no2", "no3", "no4"]
        );
    }

    #[test]
    fn test_lint_unreachable() {
        let m = module(
            r#"
def test():
    return 1
    no1
def test2():
    if x:
        return 1
    yes
def test3():
    if x:
        return
    else:
        return
    no2
    ignored
def test4():
    for x in xs:
        continue
        no3
    reachable
def test5():
    for x in xs:
        if test:
            continue
        reachable
        return
    reachable
def test6():
    fail(1)
    no4
def f():
    def g():
        return 5
    reachable
"#,
        );
        let mut res = Vec::new();
        reachable(m.codemap(), m.statement(), &mut res);
        assert_eq!(
            res.map(|x| x.problem.about()),
            &["no1", "no2", "no3", "no4"]
        );
    }

    #[test]
    fn test_lint_redundant() {
        let m = module(
            r#"
def test(): # 1
    foo
    return # bad: 3
def test2(): # 4
    return
    foo
def test3(): # 7
    if x:
        return # bad: 9
    else:
        y + 1
def test4(): # 12
    def test5():
        for x in xs:
            test
            if x:
                return
            else:
                continue # bad: 19
    test5()
def test6():
    if x:
        return
    y + 1
def test7():
    for x in xs:
        if x:
            continue
        return
"#,
        );
        let mut res = Vec::new();
        redundant(m.codemap(), m.statement(), &mut res);
        assert_eq!(
            res.map(|x| x.location.resolve_span().begin.line),
            &[3, 9, 19]
        );
    }

    #[test]
    fn test_lint_misplaced_load() {
        let m = module(
            r#"
load("a", "a")
"""
this is some comment
over multiple lines
"""
load("b", "b")

x = 1
load("c", "b")
"#,
        );
        let mut res = Vec::new();
        misplaced_load(m.codemap(), m.statement(), &mut res);
        assert_eq!(res.len(), 1);
    }

    #[test]
    fn test_lint_no_effect() {
        let src = r#"
"""
a doc block
"""
load("b", "b")

x = 1
7 ## BAD
def foo():
    [18] ## BAD
1 + 2
"#;

        let m = module(src);
        let bad = src
            .lines()
            .enumerate()
            .filter_map(|(i, x)| x.contains("## BAD").then_some(i))
            .collect::<Vec<_>>();
        let mut res = Vec::new();
        no_effect(m.codemap(), m.statement(), &mut res);
        assert_eq!(res.map(|x| x.location.resolve_span().begin.line), bad);
    }
}
