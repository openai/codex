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
use std::collections::HashSet;

use maplit::hashmap;
use once_cell::sync::Lazy;
use starlark_syntax::syntax::ast::AssignTarget;
use starlark_syntax::syntax::ast::AstAssignIdent;
use starlark_syntax::syntax::ast::AstExpr;
use starlark_syntax::syntax::ast::AstStmt;
use starlark_syntax::syntax::ast::BinOp;
use starlark_syntax::syntax::ast::DefP;
use starlark_syntax::syntax::ast::Expr;
use starlark_syntax::syntax::ast::LoadArgP;
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

#[derive(Error, Debug)]
pub(crate) enum Incompatibility {
    #[error("Type check `{0}` should be written `{1}`")]
    IncompatibleTypeCheck(String, String),
    #[error("Duplicate top-level assignment of `{0}`, first defined at {1}")]
    DuplicateTopLevelAssign(String, FileSpan),
}

impl LintWarning for Incompatibility {
    fn severity(&self) -> EvalSeverity {
        EvalSeverity::Warning
    }

    fn short_name(&self) -> &'static str {
        match self {
            Incompatibility::IncompatibleTypeCheck(..) => "incompatible-type-check",
            Incompatibility::DuplicateTopLevelAssign(..) => "duplicate-top-level-assign",
        }
    }
}

static TYPES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    hashmap![
        "bool" => "True",
        "tuple" => "()",
        "str" => "\"\"",
        "list" => "[]",
        "int" => "0"
    ]
});

fn match_bad_type_equality(
    codemap: &CodeMap,
    x: &AstExpr,
    types: &HashMap<&str, &str>,
    res: &mut Vec<LintT<Incompatibility>>,
) {
    fn lookup_type<'a>(x: &AstExpr, types: &HashMap<&str, &'a str>) -> Option<&'a str> {
        match &**x {
            Expr::Identifier(name) => types.get(name.node.ident.as_str()).copied(),
            _ => None,
        }
    }

    // Return true if this expression matches `type($x)`
    fn is_type_call(x: &AstExpr) -> bool {
        match &**x {
            Expr::Call(fun, args) if args.args.len() == 1 => match &***fun {
                Expr::Identifier(x) => x.node.ident == "type",
                _ => false,
            },
            _ => false,
        }
    }

    // If we see type(x) == y (or negated), where y is in our types table, suggest a replacement
    match &**x {
        Expr::Op(lhs, op, rhs)
            if (*op == BinOp::Equal || *op == BinOp::NotEqual) && is_type_call(lhs) =>
        {
            if let Some(replacement) = lookup_type(rhs, types) {
                res.push(LintT::new(
                    codemap,
                    x.span,
                    Incompatibility::IncompatibleTypeCheck(
                        x.to_string(),
                        format!("{}{}type({})", lhs.node, op, replacement),
                    ),
                ))
            }
        }
        _ => {}
    }
}

fn bad_type_equality(module: &AstModule, res: &mut Vec<LintT<Incompatibility>>) {
    let types = Lazy::force(&TYPES);
    fn check(
        codemap: &CodeMap,
        x: &AstExpr,
        types: &HashMap<&str, &str>,
        res: &mut Vec<LintT<Incompatibility>>,
    ) {
        match_bad_type_equality(codemap, x, types, res);
        x.visit_expr(|x| check(codemap, x, types, res));
    }
    module
        .statement()
        .visit_expr(|x| check(module.codemap(), x, types, res));
}

// Go implementation of Starlark disallows duplicate top-level assignments,
// it's likely that will become Starlark standard sooner or later, so check now.
// The one place we allow it is to export something you grabbed with load.
fn duplicate_top_level_assignment(module: &AstModule, res: &mut Vec<LintT<Incompatibility>>) {
    let mut defined = HashMap::new(); //(name, (location, is_load))
    let mut exported = HashSet::new(); // name's already exported by is_load

    fn ident<'a>(
        x: &'a AstAssignIdent,
        is_load: bool,
        codemap: &CodeMap,
        defined: &mut HashMap<&'a str, (Span, bool)>,
        res: &mut Vec<LintT<Incompatibility>>,
    ) {
        if let Some((old, _)) = defined.get(x.ident.as_str()) {
            res.push(LintT::new(
                codemap,
                x.span,
                Incompatibility::DuplicateTopLevelAssign(x.ident.clone(), codemap.file_span(*old)),
            ));
        } else {
            defined.insert(&x.ident, (x.span, is_load));
        }
    }

    fn stmt<'a>(
        x: &'a AstStmt,
        codemap: &CodeMap,
        defined: &mut HashMap<&'a str, (Span, bool)>,
        exported: &mut HashSet<&'a str>,
        res: &mut Vec<LintT<Incompatibility>>,
    ) {
        match &**x {
            Stmt::Assign(assign) => match (&assign.lhs.node, &assign.rhs.node) {
                (AssignTarget::Identifier(x), Expr::Identifier(y))
                    if x.node.ident == y.node.ident
                        && defined.get(x.node.ident.as_str()).map_or(false, |x| x.1)
                        && !exported.contains(x.node.ident.as_str()) =>
                {
                    // Normally this would be an error, but if we load()'d it, this is how we'd reexport through Starlark.
                    // But only allow one export
                    exported.insert(x.node.ident.as_str());
                }
                _ => assign
                    .lhs
                    .visit_lvalue(|x| ident(x, false, codemap, defined, res)),
            },
            Stmt::AssignModify(lhs, _, _) => {
                lhs.visit_lvalue(|x| ident(x, false, codemap, defined, res))
            }
            Stmt::Def(DefP { name, .. }) => ident(name, false, codemap, defined, res),
            Stmt::Load(load) => {
                for LoadArgP { local, .. } in &load.args {
                    ident(local, true, codemap, defined, res)
                }
            }
            // Visit statements, but don't descend under def - only top-level statements are interesting
            _ => x.visit_stmt(|x| stmt(x, codemap, defined, exported, res)),
        }
    }

    stmt(
        module.statement(),
        module.codemap(),
        &mut defined,
        &mut exported,
        res,
    )
}

pub(crate) fn lint(module: &AstModule) -> Vec<LintT<Incompatibility>> {
    let mut res = Vec::new();
    bad_type_equality(module, &mut res);
    duplicate_top_level_assignment(module, &mut res);
    res
}

#[cfg(test)]
mod tests {
    use starlark_syntax::slice_vec_ext::SliceExt;

    use super::*;
    use crate::syntax::Dialect;

    fn module(x: &str) -> AstModule {
        AstModule::parse("bad.py", x.to_owned(), &Dialect::AllOptionsInternal).unwrap()
    }

    #[test]
    fn test_lint_incompatible() {
        let mut res = Vec::new();
        bad_type_equality(
            &module(
                r#"
def foo():
    if type(x) == str and type(y) == type(list) and type(z) == foobar:
        pass
"#,
            ),
            &mut res,
        );
        assert_eq!(
            res.map(|x| x.to_string()),
            &[
                "bad.py:3:8-22: Type check `(type(x) == str)` should be written `type(x) == type(\"\")`"
            ]
        );
    }

    #[test]
    fn test_lint_duplicate_top_level_assign() {
        let m = module(
            r#"
load("file", "foo", "no3", "no4")
no1 = 1
no1 = 4
no1 += 8
foo = foo # Starlark reexport
no3 = no3
no3 = no3
no4 = no4 + 1
def no2(): pass
def no2():
    x = 1
    x += 1
    return x
"#,
        );
        let mut res = Vec::new();
        duplicate_top_level_assignment(&m, &mut res);
        let mut res = res.map(|x| match &x.problem {
            Incompatibility::DuplicateTopLevelAssign(x, _) => x,
            _ => panic!("Unexpected lint"),
        });
        res.sort();
        assert_eq!(res, &["no1", "no1", "no2", "no3", "no4"])
    }
}
