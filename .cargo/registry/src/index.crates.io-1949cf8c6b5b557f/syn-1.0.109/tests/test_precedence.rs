#![cfg(not(syn_disable_nightly_tests))]
#![cfg(not(miri))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]
#![allow(
    clippy::explicit_deref_methods,
    clippy::manual_assert,
    clippy::match_wildcard_for_single_variants,
    clippy::too_many_lines
)]

//! The tests in this module do the following:
//!
//! 1. Parse a given expression in both `syn` and `librustc`.
//! 2. Fold over the expression adding brackets around each subexpression (with
//!    some complications - see the `syn_brackets` and `librustc_brackets`
//!    methods).
//! 3. Serialize the `syn` expression back into a string, and re-parse it with
//!    `librustc`.
//! 4. Respan all of the expressions, replacing the spans with the default
//!    spans.
//! 5. Compare the expressions with one another, if they are not equal fail.

extern crate rustc_ast;
extern crate rustc_data_structures;
extern crate rustc_span;
extern crate thin_vec;

use crate::common::eq::SpanlessEq;
use crate::common::parse;
use quote::quote;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use regex::Regex;
use rustc_ast::ast;
use rustc_ast::ptr::P;
use rustc_span::edition::Edition;
use std::fs;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::{DirEntry, WalkDir};

#[macro_use]
mod macros;

#[allow(dead_code)]
mod common;

mod repo;

/// Test some pre-set expressions chosen by us.
#[test]
fn test_simple_precedence() {
    const EXPRS: &[&str] = &[
        "1 + 2 * 3 + 4",
        "1 + 2 * ( 3 + 4 )",
        "{ for i in r { } *some_ptr += 1; }",
        "{ loop { break 5; } }",
        "{ if true { () }.mthd() }",
        "{ for i in unsafe { 20 } { } }",
    ];

    let mut failed = 0;

    for input in EXPRS {
        let expr = if let Some(expr) = parse::syn_expr(input) {
            expr
        } else {
            failed += 1;
            continue;
        };

        let pf = match test_expressions(Edition::Edition2018, vec![expr]) {
            (1, 0) => "passed",
            (0, 1) => {
                failed += 1;
                "failed"
            }
            _ => unreachable!(),
        };
        errorf!("=== {}: {}\n", input, pf);
    }

    if failed > 0 {
        panic!("Failed {} tests", failed);
    }
}

/// Test expressions from rustc, like in `test_round_trip`.
#[test]
fn test_rustc_precedence() {
    common::rayon_init();
    repo::clone_rust();
    let abort_after = common::abort_after();
    if abort_after == 0 {
        panic!("Skipping all precedence tests");
    }

    let passed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    // 2018 edition is hard
    let edition_regex = Regex::new(r"\b(async|try)[!(]").unwrap();

    WalkDir::new("tests/rust")
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter()
        .filter_entry(repo::base_dir_filter)
        .collect::<Result<Vec<DirEntry>, walkdir::Error>>()
        .unwrap()
        .into_par_iter()
        .for_each(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return;
            }

            let content = fs::read_to_string(path).unwrap();
            let content = edition_regex.replace_all(&content, "_$0");

            let (l_passed, l_failed) = match syn::parse_file(&content) {
                Ok(file) => {
                    let edition = repo::edition(path).parse().unwrap();
                    let exprs = collect_exprs(file);
                    test_expressions(edition, exprs)
                }
                Err(msg) => {
                    errorf!("syn failed to parse\n{:?}\n", msg);
                    (0, 1)
                }
            };

            errorf!(
                "=== {}: {} passed | {} failed\n",
                path.display(),
                l_passed,
                l_failed
            );

            passed.fetch_add(l_passed, Ordering::Relaxed);
            let prev_failed = failed.fetch_add(l_failed, Ordering::Relaxed);

            if prev_failed + l_failed >= abort_after {
                process::exit(1);
            }
        });

    let passed = passed.load(Ordering::Relaxed);
    let failed = failed.load(Ordering::Relaxed);

    errorf!("\n===== Precedence Test Results =====\n");
    errorf!("{} passed | {} failed\n", passed, failed);

    if failed > 0 {
        panic!("{} failures", failed);
    }
}

fn test_expressions(edition: Edition, exprs: Vec<syn::Expr>) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;

    rustc_span::create_session_if_not_set_then(edition, |_| {
        for expr in exprs {
            let raw = quote!(#expr).to_string();

            let librustc_ast = if let Some(e) = librustc_parse_and_rewrite(&raw) {
                e
            } else {
                failed += 1;
                errorf!("\nFAIL - librustc failed to parse raw\n");
                continue;
            };

            let syn_expr = syn_brackets(expr);
            let syn_ast = if let Some(e) = parse::librustc_expr(&quote!(#syn_expr).to_string()) {
                e
            } else {
                failed += 1;
                errorf!("\nFAIL - librustc failed to parse bracketed\n");
                continue;
            };

            if SpanlessEq::eq(&syn_ast, &librustc_ast) {
                passed += 1;
            } else {
                failed += 1;
                errorf!("\nFAIL\n{:?}\n!=\n{:?}\n", syn_ast, librustc_ast);
            }
        }
    });

    (passed, failed)
}

fn librustc_parse_and_rewrite(input: &str) -> Option<P<ast::Expr>> {
    parse::librustc_expr(input).and_then(librustc_brackets)
}

/// Wrap every expression which is not already wrapped in parens with parens, to
/// reveal the precedence of the parsed expressions, and produce a stringified
/// form of the resulting expression.
///
/// This method operates on librustc objects.
fn librustc_brackets(mut librustc_expr: P<ast::Expr>) -> Option<P<ast::Expr>> {
    use rustc_ast::ast::{
        Attribute, Block, BorrowKind, Expr, ExprField, ExprKind, GenericArg, Local, LocalKind, Pat,
        Stmt, StmtKind, StructExpr, StructRest, Ty,
    };
    use rustc_ast::mut_visit::{noop_visit_generic_arg, noop_visit_local, MutVisitor};
    use rustc_data_structures::map_in_place::MapInPlace;
    use rustc_span::DUMMY_SP;
    use std::mem;
    use std::ops::DerefMut;
    use thin_vec::ThinVec;

    struct BracketsVisitor {
        failed: bool,
    }

    fn flat_map_field<T: MutVisitor>(mut f: ExprField, vis: &mut T) -> Vec<ExprField> {
        if f.is_shorthand {
            noop_visit_expr(&mut f.expr, vis);
        } else {
            vis.visit_expr(&mut f.expr);
        }
        vec![f]
    }

    fn flat_map_stmt<T: MutVisitor>(stmt: Stmt, vis: &mut T) -> Vec<Stmt> {
        let kind = match stmt.kind {
            // Don't wrap toplevel expressions in statements.
            StmtKind::Expr(mut e) => {
                noop_visit_expr(&mut e, vis);
                StmtKind::Expr(e)
            }
            StmtKind::Semi(mut e) => {
                noop_visit_expr(&mut e, vis);
                StmtKind::Semi(e)
            }
            s => s,
        };

        vec![Stmt { kind, ..stmt }]
    }

    fn noop_visit_expr<T: MutVisitor>(e: &mut Expr, vis: &mut T) {
        use rustc_ast::mut_visit::{noop_visit_expr, visit_attrs};
        match &mut e.kind {
            ExprKind::AddrOf(BorrowKind::Raw, ..) => {}
            ExprKind::Struct(expr) => {
                let StructExpr {
                    qself,
                    path,
                    fields,
                    rest,
                } = expr.deref_mut();
                vis.visit_qself(qself);
                vis.visit_path(path);
                fields.flat_map_in_place(|field| flat_map_field(field, vis));
                if let StructRest::Base(rest) = rest {
                    vis.visit_expr(rest);
                }
                vis.visit_id(&mut e.id);
                vis.visit_span(&mut e.span);
                visit_attrs(&mut e.attrs, vis);
            }
            _ => noop_visit_expr(e, vis),
        }
    }

    impl MutVisitor for BracketsVisitor {
        fn visit_expr(&mut self, e: &mut P<Expr>) {
            match e.kind {
                ExprKind::ConstBlock(..) => {}
                _ => noop_visit_expr(e, self),
            }
            match e.kind {
                ExprKind::If(..) | ExprKind::Block(..) | ExprKind::Let(..) => {}
                _ => {
                    let inner = mem::replace(
                        e,
                        P(Expr {
                            id: ast::DUMMY_NODE_ID,
                            kind: ExprKind::Err,
                            span: DUMMY_SP,
                            attrs: ThinVec::new(),
                            tokens: None,
                        }),
                    );
                    e.kind = ExprKind::Paren(inner);
                }
            }
        }

        fn visit_generic_arg(&mut self, arg: &mut GenericArg) {
            match arg {
                // Don't wrap unbraced const generic arg as that's invalid syntax.
                GenericArg::Const(anon_const) => {
                    if let ExprKind::Block(..) = &mut anon_const.value.kind {
                        noop_visit_expr(&mut anon_const.value, self);
                    }
                }
                _ => noop_visit_generic_arg(arg, self),
            }
        }

        fn visit_block(&mut self, block: &mut P<Block>) {
            self.visit_id(&mut block.id);
            block
                .stmts
                .flat_map_in_place(|stmt| flat_map_stmt(stmt, self));
            self.visit_span(&mut block.span);
        }

        fn visit_local(&mut self, local: &mut P<Local>) {
            match local.kind {
                LocalKind::InitElse(..) => {}
                _ => noop_visit_local(local, self),
            }
        }

        // We don't want to look at expressions that might appear in patterns or
        // types yet. We'll look into comparing those in the future. For now
        // focus on expressions appearing in other places.
        fn visit_pat(&mut self, pat: &mut P<Pat>) {
            _ = pat;
        }

        fn visit_ty(&mut self, ty: &mut P<Ty>) {
            _ = ty;
        }

        fn visit_attribute(&mut self, attr: &mut Attribute) {
            _ = attr;
        }
    }

    let mut folder = BracketsVisitor { failed: false };
    folder.visit_expr(&mut librustc_expr);
    if folder.failed {
        None
    } else {
        Some(librustc_expr)
    }
}

/// Wrap every expression which is not already wrapped in parens with parens, to
/// reveal the precedence of the parsed expressions, and produce a stringified
/// form of the resulting expression.
fn syn_brackets(syn_expr: syn::Expr) -> syn::Expr {
    use syn::fold::{fold_expr, fold_generic_argument, fold_generic_method_argument, Fold};
    use syn::{token, Expr, ExprParen, GenericArgument, GenericMethodArgument, Pat, Stmt, Type};

    struct ParenthesizeEveryExpr;
    impl Fold for ParenthesizeEveryExpr {
        fn fold_expr(&mut self, expr: Expr) -> Expr {
            match expr {
                Expr::Group(_) => unreachable!(),
                Expr::If(..) | Expr::Unsafe(..) | Expr::Block(..) | Expr::Let(..) => {
                    fold_expr(self, expr)
                }
                _ => Expr::Paren(ExprParen {
                    attrs: Vec::new(),
                    expr: Box::new(fold_expr(self, expr)),
                    paren_token: token::Paren::default(),
                }),
            }
        }

        fn fold_generic_argument(&mut self, arg: GenericArgument) -> GenericArgument {
            match arg {
                GenericArgument::Const(arg) => GenericArgument::Const(match arg {
                    Expr::Block(_) => fold_expr(self, arg),
                    // Don't wrap unbraced const generic arg as that's invalid syntax.
                    _ => arg,
                }),
                _ => fold_generic_argument(self, arg),
            }
        }

        fn fold_generic_method_argument(
            &mut self,
            arg: GenericMethodArgument,
        ) -> GenericMethodArgument {
            match arg {
                GenericMethodArgument::Const(arg) => GenericMethodArgument::Const(match arg {
                    Expr::Block(_) => fold_expr(self, arg),
                    // Don't wrap unbraced const generic arg as that's invalid syntax.
                    _ => arg,
                }),
                _ => fold_generic_method_argument(self, arg),
            }
        }

        fn fold_stmt(&mut self, stmt: Stmt) -> Stmt {
            match stmt {
                // Don't wrap toplevel expressions in statements.
                Stmt::Expr(e) => Stmt::Expr(fold_expr(self, e)),
                Stmt::Semi(e, semi) => {
                    if let Expr::Verbatim(_) = e {
                        Stmt::Semi(e, semi)
                    } else {
                        Stmt::Semi(fold_expr(self, e), semi)
                    }
                }
                s => s,
            }
        }

        // We don't want to look at expressions that might appear in patterns or
        // types yet. We'll look into comparing those in the future. For now
        // focus on expressions appearing in other places.
        fn fold_pat(&mut self, pat: Pat) -> Pat {
            pat
        }

        fn fold_type(&mut self, ty: Type) -> Type {
            ty
        }
    }

    let mut folder = ParenthesizeEveryExpr;
    folder.fold_expr(syn_expr)
}

/// Walk through a crate collecting all expressions we can find in it.
fn collect_exprs(file: syn::File) -> Vec<syn::Expr> {
    use syn::fold::Fold;
    use syn::punctuated::Punctuated;
    use syn::{token, ConstParam, Expr, ExprTuple, Path};

    struct CollectExprs(Vec<Expr>);
    impl Fold for CollectExprs {
        fn fold_expr(&mut self, expr: Expr) -> Expr {
            match expr {
                Expr::Verbatim(_) => {}
                _ => self.0.push(expr),
            }

            Expr::Tuple(ExprTuple {
                attrs: vec![],
                elems: Punctuated::new(),
                paren_token: token::Paren::default(),
            })
        }

        fn fold_path(&mut self, path: Path) -> Path {
            // Skip traversing into const generic path arguments
            path
        }

        fn fold_const_param(&mut self, const_param: ConstParam) -> ConstParam {
            const_param
        }
    }

    let mut folder = CollectExprs(vec![]);
    folder.fold_file(file);
    folder.0
}
