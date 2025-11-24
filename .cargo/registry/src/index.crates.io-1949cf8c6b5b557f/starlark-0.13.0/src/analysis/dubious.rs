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

use std::collections::HashMap;

use starlark_syntax::syntax::ast::AstExpr;
use starlark_syntax::syntax::ast::AstLiteral;
use starlark_syntax::syntax::ast::AstStmt;
use starlark_syntax::syntax::ast::Expr;
use starlark_syntax::syntax::ast::Stmt;
use starlark_syntax::syntax::module::AstModuleFields;
use thiserror::Error;

use crate::analysis::types::LintT;
use crate::analysis::types::LintWarning;
use crate::analysis::EvalSeverity;
use crate::codemap::CodeMap;
use crate::codemap::FileSpan;
use crate::codemap::Span;
use crate::syntax::AstModule;
use crate::values::types::int::int_or_big::StarlarkInt;
use crate::values::types::num::value::NumRef;

#[derive(Error, Debug)]
pub(crate) enum Dubious {
    #[error("Duplicate dictionary key `{0}`, also used at {1}")]
    DuplicateKey(String, FileSpan),
    #[error("Variable `{0}` will either do nothing or fail if uninitialised")]
    IdentifierAsStatement(String),
}

impl LintWarning for Dubious {
    fn severity(&self) -> EvalSeverity {
        EvalSeverity::Warning
    }

    fn short_name(&self) -> &'static str {
        match self {
            Dubious::DuplicateKey(..) => "duplicate-key",
            Dubious::IdentifierAsStatement(..) => "ident-as-statement",
        }
    }
}

// Go implementation of Starlark disallows duplicate top-level assignments,
// it's likely that will become Starlark standard sooner or later, so check now.
// The one place we allow it is to export something you grabbed with load.
fn duplicate_dictionary_key(module: &AstModule, res: &mut Vec<LintT<Dubious>>) {
    #[derive(PartialEq, Eq, Hash)]
    enum Key<'a> {
        Int(StarlarkInt),
        Float(u64),
        String(&'a str),
        Identifier(&'a str),
    }

    fn to_key<'a>(x: &'a AstExpr) -> Option<(Key<'a>, Span)> {
        match &**x {
            Expr::Literal(x) => match x {
                AstLiteral::Int(x) => Some((Key::Int(StarlarkInt::from(x.node.clone())), x.span)),
                AstLiteral::Float(x) => {
                    let n = NumRef::from(x.node);
                    if let Some(i) = n.as_int() {
                        // make an integer float always collide with other ints
                        Some((Key::Int(StarlarkInt::from(i)), x.span))
                    } else {
                        // use bits representation of float to be able to always compare them for equality
                        // First normalise -0.0
                        let v = if x.node == 0.0 { 0.0 } else { x.node };
                        Some((Key::Float(v.to_bits()), x.span))
                    }
                }
                AstLiteral::String(x) => Some((Key::String(&x.node), x.span)),
                AstLiteral::Ellipsis => None,
            },
            Expr::Identifier(x) => Some((Key::Identifier(&x.node.ident), x.span)),
            _ => None,
        }
    }

    fn expr<'a>(x: &'a AstExpr, codemap: &CodeMap, res: &mut Vec<LintT<Dubious>>) {
        match &**x {
            Expr::Dict(args) => {
                let mut seen = HashMap::new();
                for (key, _) in args {
                    if let Some((key_id, pos)) = to_key(key) {
                        if let Some(old) = seen.insert(key_id, pos) {
                            res.push(LintT::new(
                                codemap,
                                old,
                                Dubious::DuplicateKey(key.to_string(), codemap.file_span(pos)),
                            ))
                        }
                    }
                }
            }
            _ => {}
        }
        x.visit_expr(|x| expr(x, codemap, res));
    }

    module
        .statement()
        .visit_expr(|x| expr(x, module.codemap(), res))
}

fn identifier_as_statement(module: &AstModule, res: &mut Vec<LintT<Dubious>>) {
    fn stmt<'a>(x: &'a AstStmt, codemap: &CodeMap, res: &mut Vec<LintT<Dubious>>) {
        match &**x {
            Stmt::Expression(x) => match &**x {
                Expr::Identifier(x) => res.push(LintT::new(
                    codemap,
                    x.span,
                    Dubious::IdentifierAsStatement(x.node.ident.clone()),
                )),
                _ => {}
            },
            _ => x.visit_stmt(|x| stmt(x, codemap, res)),
        }
    }

    stmt(module.statement(), module.codemap(), res)
}

pub(crate) fn lint(module: &AstModule) -> Vec<LintT<Dubious>> {
    let mut res = Vec::new();
    duplicate_dictionary_key(module, &mut res);
    identifier_as_statement(module, &mut res);
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

    impl Dubious {
        fn about(&self) -> &String {
            match self {
                Dubious::DuplicateKey(x, _) => x,
                Dubious::IdentifierAsStatement(x) => x,
            }
        }
    }

    #[test]
    fn test_lint_duplicate_keys() {
        let m = module(
            r#"
{'no1': 1, 'no1': 2}
{42: 1, 78: 9, 'no2': 100, 42: 6, 'no2': 8}
{123.0: "f", 123: "i"}
{0.25: "frac", 25e-2: "exp"}

# Variables can't change as a result of expression evaluation,
# so it's always an error if you see the same expression
{no3: 1, no4: 2, yes: 3, no3: 1, no3: 3, no4: 8}

# Functions can change each time round, so don't lint on them.
{f(): 1, f(): 2}
"#,
        );
        let mut res = Vec::new();
        duplicate_dictionary_key(&m, &mut res);
        assert_eq!(
            res.map(|x| x.problem.about()),
            &[
                "\"no1\"", "42", "\"no2\"", "123", "0.25", "no3", "no3", "no4"
            ]
        );
    }

    #[test]
    fn test_lint_identifier_as_statement() {
        let m = module(
            r#"
no1
def foo():
    f(yes)
    no2
"#,
        );
        let mut res = Vec::new();
        identifier_as_statement(&m, &mut res);
        assert_eq!(res.map(|x| x.problem.about()), &["no1", "no2"]);
    }
}
