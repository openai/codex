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

use starlark_syntax::syntax::ast::Argument;
use starlark_syntax::syntax::ast::AstExpr;
use starlark_syntax::syntax::ast::Expr;
use starlark_syntax::syntax::module::AstModuleFields;
use thiserror::Error;

use crate::analysis::types::LintT;
use crate::analysis::types::LintWarning;
use crate::analysis::EvalSeverity;
use crate::codemap::CodeMap;
use crate::syntax::AstModule;

#[derive(Error, Debug)]
pub(crate) enum Performance {
    #[error("Dict copy `{0}` is more efficient as `{1}`")]
    DictWithoutStarStar(String, String),

    #[error(
        "`{0}` eagerly evaluates all items in the iterable, and allocates an array for the results. Prefer using a for-loop."
    )]
    EagerAndInefficientBoolCheck(String),

    #[error("`{0}` allocates a new {1} for the results. Prefer using a for-loop.")]
    InefficientBoolCheck(String, String),
}

impl LintWarning for Performance {
    fn severity(&self) -> EvalSeverity {
        EvalSeverity::Warning
    }

    fn short_name(&self) -> &'static str {
        match self {
            Performance::DictWithoutStarStar(..) => "dict-without-star-star",
            Performance::EagerAndInefficientBoolCheck(..) => "eager-and-inefficient-bool-check",
            Performance::InefficientBoolCheck(..) => "inefficient-bool-check",
        }
    }
}

fn match_dict_copy(codemap: &CodeMap, x: &AstExpr, res: &mut Vec<LintT<Performance>>) {
    // If we see `dict(**x)` suggest `dict(x)`
    match &**x {
        Expr::Call(fun, args) if args.args.len() == 1 => match (&***fun, &*args.args[0]) {
            (Expr::Identifier(f), Argument::KwArgs(arg)) if f.node.ident == "dict" => {
                res.push(LintT::new(
                    codemap,
                    x.span,
                    Performance::DictWithoutStarStar(x.to_string(), format!("dict({})", arg.node)),
                ))
            }
            _ => {}
        },
        _ => {}
    }
}

fn match_inefficient_bool_check(codemap: &CodeMap, x: &AstExpr, res: &mut Vec<LintT<Performance>>) {
    match &**x {
        Expr::Call(fun, args) if args.args.len() == 1 => match (&***fun, &*args.args[0]) {
            (Expr::Identifier(f), Argument::Positional(arg))
                if f.node.ident == "any" || f.node.ident == "all" =>
            {
                match &**arg {
                    // any([blah for blah in blahs])
                    Expr::ListComprehension(_, _, _) | Expr::DictComprehension(_, _, _) => res
                        .push(LintT::new(
                            codemap,
                            x.span,
                            Performance::EagerAndInefficientBoolCheck(f.node.ident.clone()),
                        )),
                    // any(list(_get_some_dict()))
                    Expr::Call(any_call, _) => match &***any_call {
                        Expr::Identifier(any_id)
                            if any_id.node.ident == "dict" || any_id.node.ident == "list" =>
                        {
                            res.push(LintT::new(
                                codemap,
                                x.span,
                                Performance::InefficientBoolCheck(
                                    x.to_string(),
                                    any_id.node.ident.clone(),
                                ),
                            ))
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn check_call_expr(module: &AstModule, res: &mut Vec<LintT<Performance>>) {
    fn check(codemap: &CodeMap, x: &AstExpr, res: &mut Vec<LintT<Performance>>) {
        match_dict_copy(codemap, x, res);
        match_inefficient_bool_check(codemap, x, res);
        x.visit_expr(|x| check(codemap, x, res));
    }
    module
        .statement()
        .visit_expr(|x| check(module.codemap(), x, res));
}

pub(crate) fn lint(module: &AstModule) -> Vec<LintT<Performance>> {
    let mut res = Vec::new();
    check_call_expr(module, &mut res);
    res
}

#[cfg(test)]
mod tests {
    use starlark_syntax::slice_vec_ext::SliceExt;

    use super::*;
    use crate::syntax::Dialect;

    fn module(x: &str) -> AstModule {
        AstModule::parse("bad.bzl", x.to_owned(), &Dialect::AllOptionsInternal).unwrap()
    }

    #[test]
    fn test_lint_matches_dict_issue() {
        let mut res = Vec::new();
        check_call_expr(
            &module(
                r#"
def foo(extra, **kwargs):
    x = dict(**kwargs)
    y = dict(extra)
    return (x,y)
"#,
            ),
            &mut res,
        );
        assert_eq!(
            res.map(|x| x.to_string()),
            &["bad.bzl:3:9-23: Dict copy `dict(**kwargs)` is more efficient as `dict(kwargs)`"]
        );
    }

    #[test]
    fn test_lint_matches_any_function() {
        let mut res = Vec::new();
        check_call_expr(
            &module(
                r#"
def foo(items):
    a = all(items)
    b = all([item for item in items])
    c = any([item for item in items])
    d = all({"a": a for a in []})
    e = any(list({}))
    f = all(dict([]))
    return (a,b,c,d,e,f)
"#,
            ),
            &mut res,
        );
        assert_eq!(
            res.map(|x| x.to_string()),
            &[
                "bad.bzl:4:9-38: `all` eagerly evaluates all items in the iterable, and allocates an array for the results. Prefer using a for-loop.",
                "bad.bzl:5:9-38: `any` eagerly evaluates all items in the iterable, and allocates an array for the results. Prefer using a for-loop.",
                "bad.bzl:6:9-34: `all` eagerly evaluates all items in the iterable, and allocates an array for the results. Prefer using a for-loop.",
                "bad.bzl:7:9-22: `any(list({}))` allocates a new list for the results. Prefer using a for-loop.",
                "bad.bzl:8:9-22: `all(dict([]))` allocates a new dict for the results. Prefer using a for-loop."
            ]
        );
    }
}
