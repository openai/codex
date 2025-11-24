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

use std::collections::HashSet;

use starlark_syntax::syntax::ast::AssignP;
use starlark_syntax::syntax::ast::AssignTarget;
use starlark_syntax::syntax::ast::AstExpr;
use starlark_syntax::syntax::ast::AstStmt;
use starlark_syntax::syntax::ast::DefP;
use starlark_syntax::syntax::ast::Expr;
use starlark_syntax::syntax::ast::Stmt;
use starlark_syntax::syntax::module::AstModuleFields;
use thiserror::Error;

use crate::analysis::types::LintT;
use crate::analysis::types::LintWarning;
use crate::analysis::EvalSeverity;
use crate::codemap::CodeMap;
use crate::syntax::AstModule;

#[derive(Error, Debug)]
pub(crate) enum UnderscoreWarning {
    #[error("Underscore definitions should be simple `{0}`")]
    UnderscoreDefinition(String),
    #[error("Used ignored variable `{0}`")]
    UsingIgnored(String),
}

impl LintWarning for UnderscoreWarning {
    fn severity(&self) -> EvalSeverity {
        EvalSeverity::Disabled
    }

    fn short_name(&self) -> &'static str {
        match self {
            UnderscoreWarning::UnderscoreDefinition(..) => "underscore-definition",
            UnderscoreWarning::UsingIgnored(..) => "using-ignored",
        }
    }
}

pub(crate) fn lint(module: &AstModule) -> Vec<LintT<UnderscoreWarning>> {
    let mut res = Vec::new();
    inappropriate_underscore(module.codemap(), module.statement(), true, &mut res);
    use_ignored(module.codemap(), module.statement(), &mut res);
    res
}

// There's no reason to make a def or lambda and give it an underscore name not at the top level
fn inappropriate_underscore(
    codemap: &CodeMap,
    x: &AstStmt,
    top: bool,
    res: &mut Vec<LintT<UnderscoreWarning>>,
) {
    // Is this value allowed as an assignment to a boring identifier - just tuple of vars and var
    fn is_allowed(x: &AstExpr) -> bool {
        match &**x {
            Expr::Tuple(xs) if !xs.is_empty() => {
                xs.iter().all(|x| matches!(&**x, Expr::Identifier(..)))
            }
            Expr::Identifier(..) => true,
            _ => false,
        }
    }

    match &**x {
        Stmt::Def(DefP { name, body, .. }) => {
            if !top && name.ident.starts_with('_') {
                res.push(LintT::new(
                    codemap,
                    name.span,
                    UnderscoreWarning::UnderscoreDefinition(name.ident.clone()),
                ))
            }
            inappropriate_underscore(codemap, body, false, res)
        }
        Stmt::Assign(assign) if !top => {
            if let AssignTarget::Identifier(name) = &assign.lhs.node {
                if name.ident.starts_with('_') && !is_allowed(&assign.rhs) {
                    res.push(LintT::new(
                        codemap,
                        name.span,
                        UnderscoreWarning::UnderscoreDefinition(name.node.ident.clone()),
                    ))
                }
            }
        }
        _ => x.visit_stmt(|x| inappropriate_underscore(codemap, x, top, res)),
    }
}

// Don't want to use a variable that has been defined to be ignored
fn use_ignored(codemap: &CodeMap, x: &AstStmt, res: &mut Vec<LintT<UnderscoreWarning>>) {
    // we are ok with using things that were defined at the top level, but not nested
    fn root_definitions<'a>(x: &'a AstStmt, res: &mut HashSet<&'a str>) {
        match &**x {
            Stmt::Assign(AssignP { lhs: x, .. }) | Stmt::AssignModify(x, _, _) => {
                x.visit_lvalue(|x| {
                    res.insert(x.ident.as_str());
                });
            }
            Stmt::Def(x) => {
                res.insert(x.name.ident.as_str());
            }
            Stmt::Load(xs) => {
                for x in &xs.args {
                    res.insert(x.local.ident.as_str());
                }
            }
            _ => x.visit_stmt(|x| root_definitions(x, res)),
        }
    }

    fn is_ignored(x: &str) -> bool {
        // we want things like __internal__ for builtin things to expose themselves quietly
        x.starts_with('_') && !(x.starts_with("__") && x.ends_with("__"))
    }

    fn check_expr(
        codemap: &CodeMap,
        x: &AstExpr,
        roots: &HashSet<&str>,
        res: &mut Vec<LintT<UnderscoreWarning>>,
    ) {
        match &**x {
            Expr::Identifier(x) => {
                if is_ignored(x.ident.as_str()) && !roots.contains(x.ident.as_str()) {
                    res.push(LintT::new(
                        codemap,
                        x.span,
                        UnderscoreWarning::UsingIgnored(x.node.ident.clone()),
                    ));
                }
            }
            _ => x.visit_expr(|x| check_expr(codemap, x, roots, res)),
        }
    }

    let mut roots = HashSet::new();
    root_definitions(x, &mut roots);
    x.visit_expr(|x| check_expr(codemap, x, &roots, res));
}

#[cfg(test)]
mod tests {
    use starlark_syntax::slice_vec_ext::SliceExt;

    use super::*;
    use crate::syntax::Dialect;

    impl UnderscoreWarning {
        fn about(&self) -> &String {
            match self {
                UnderscoreWarning::UnderscoreDefinition(x) => x,
                UnderscoreWarning::UsingIgnored(x) => x,
            }
        }
    }

    fn module(x: &str) -> AstModule {
        AstModule::parse("X", x.to_owned(), &Dialect::AllOptionsInternal).unwrap()
    }

    #[test]
    fn test_lint_inappropriate_underscore() {
        let m = module(
            r#"
def _ok():
    def _no1(foo):
        _no2 = lambda x: x
        _no3 = 8
        _unused = foo
"#,
        );
        let mut res = Vec::new();
        inappropriate_underscore(m.codemap(), m.statement(), true, &mut res);
        let mut res = res.map(|x| x.problem.about());
        res.sort();
        assert_eq!(res, &["_no1", "_no2", "_no3"])
    }

    #[test]
    fn test_lint_use_ignored() {
        let m = module(
            r#"
def _foo(): pass
_allowed = 1
_missed = 1
def bar():
    def _no1(): pass
    _foo()
    _no1()
    _no2 = 1
    print(_no2)
    _no3 = 1
    _missed = 7
    # Could argue that missed should be an error, since it shadows
    print(_missed)
    print(_allowed)
    def deeper():
        print(_no3)
        _foo(__internal__)
"#,
        );
        let mut res = Vec::new();
        use_ignored(m.codemap(), m.statement(), &mut res);
        let mut res = res.map(|x| x.problem.about());
        res.sort();
        assert_eq!(res, &["_no1", "_no2", "_no3"])
    }
}
