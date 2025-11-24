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

use crate::syntax::ast::AstPayload;
use crate::syntax::ast::AstStmtP;
use crate::syntax::ast::StmtP;

/// List the top-level statements in the AST.
pub fn top_level_stmts<P: AstPayload>(top: &AstStmtP<P>) -> Vec<&AstStmtP<P>> {
    fn f<'a, P: AstPayload>(ast: &'a AstStmtP<P>, res: &mut Vec<&'a AstStmtP<P>>) {
        match &**ast {
            StmtP::Statements(xs) => {
                for x in xs {
                    f(x, res);
                }
            }
            _ => res.push(ast),
        }
    }

    let mut res = Vec::new();
    f(top, &mut res);
    res
}

/// List the top-level statements in the AST.
pub fn top_level_stmts_mut<P: AstPayload>(top: &mut AstStmtP<P>) -> Vec<&mut AstStmtP<P>> {
    fn f<'a, P: AstPayload>(ast: &'a mut AstStmtP<P>, res: &mut Vec<&'a mut AstStmtP<P>>) {
        match &mut **ast {
            StmtP::Statements(_) => {
                if let StmtP::Statements(xs) = &mut **ast {
                    for x in xs {
                        f(x, res);
                    }
                } else {
                    unreachable!("Work around borrow checker");
                }
            }
            _ => res.push(ast),
        }
    }

    let mut res = Vec::new();
    f(top, &mut res);
    res
}
