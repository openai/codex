// This test does the following for every file in the rust-lang/rust repo:
//
// 1. Parse the file using syn into a syn::File.
// 2. Extract every syn::Expr from the file.
// 3. Print each expr to a string of source code.
// 4. Parse the source code using librustc_parse into a rustc_ast::Expr.
// 5. For both the syn::Expr and rustc_ast::Expr, crawl the syntax tree to
//    insert parentheses surrounding every subexpression.
// 6. Serialize the fully parenthesized syn::Expr to a string of source code.
// 7. Parse the fully parenthesized source code using librustc_parse.
// 8. Compare the rustc_ast::Expr resulting from parenthesizing using rustc data
//    structures vs syn data structures, ignoring spans. If they agree, rustc's
//    parser and syn's parser have identical handling of expression precedence.

#![cfg(not(syn_disable_nightly_tests))]
#![cfg(not(miri))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]
#![allow(
    clippy::blocks_in_conditions,
    clippy::doc_markdown,
    clippy::elidable_lifetime_names,
    clippy::explicit_deref_methods,
    clippy::let_underscore_untyped,
    clippy::manual_assert,
    clippy::manual_let_else,
    clippy::match_like_matches_macro,
    clippy::match_wildcard_for_single_variants,
    clippy::needless_lifetimes,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unnecessary_box_returns
)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_span;
extern crate smallvec;
extern crate thin_vec;

use crate::common::eq::SpanlessEq;
use crate::common::parse;
use quote::ToTokens;
use rustc_ast::ast;
use rustc_ast_pretty::pprust;
use rustc_span::edition::Edition;
use std::fs;
use std::mem;
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use syn::parse::Parser as _;

#[macro_use]
mod macros;

mod common;
mod repo;

#[path = "../src/scan_expr.rs"]
mod scan_expr;

#[test]
fn test_rustc_precedence() {
    repo::rayon_init();
    repo::clone_rust();
    let abort_after = repo::abort_after();
    if abort_after == 0 {
        panic!("skipping all precedence tests");
    }

    let passed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    repo::for_each_rust_file(|path| {
        let content = fs::read_to_string(path).unwrap();

        let (l_passed, l_failed) = match syn::parse_file(&content) {
            Ok(file) => {
                let edition = repo::edition(path).parse().unwrap();
                let exprs = collect_exprs(file);
                let (l_passed, l_failed) = test_expressions(path, edition, exprs);
                errorf!(
                    "=== {}: {} passed | {} failed\n",
                    path.display(),
                    l_passed,
                    l_failed,
                );
                (l_passed, l_failed)
            }
            Err(msg) => {
                errorf!("\nFAIL {} - syn failed to parse: {}\n", path.display(), msg);
                (0, 1)
            }
        };

        passed.fetch_add(l_passed, Ordering::Relaxed);
        let prev_failed = failed.fetch_add(l_failed, Ordering::Relaxed);

        if prev_failed + l_failed >= abort_after {
            process::exit(1);
        }
    });

    let passed = passed.into_inner();
    let failed = failed.into_inner();

    errorf!("\n===== Precedence Test Results =====\n");
    errorf!("{} passed | {} failed\n", passed, failed);

    if failed > 0 {
        panic!("{} failures", failed);
    }
}

fn test_expressions(path: &Path, edition: Edition, exprs: Vec<syn::Expr>) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;

    rustc_span::create_session_if_not_set_then(edition, |_| {
        for expr in exprs {
            let expr_tokens = expr.to_token_stream();
            let source_code = expr_tokens.to_string();
            let librustc_ast = if let Some(e) = librustc_parse_and_rewrite(&source_code) {
                e
            } else {
                failed += 1;
                errorf!(
                    "\nFAIL {} - librustc failed to parse original\n",
                    path.display(),
                );
                continue;
            };

            let syn_parenthesized_code =
                syn_parenthesize(expr.clone()).to_token_stream().to_string();
            let syn_ast = if let Some(e) = parse::librustc_expr(&syn_parenthesized_code) {
                e
            } else {
                failed += 1;
                errorf!(
                    "\nFAIL {} - librustc failed to parse parenthesized\n",
                    path.display(),
                );
                continue;
            };

            if !SpanlessEq::eq(&syn_ast, &librustc_ast) {
                failed += 1;
                let syn_pretty = pprust::expr_to_string(&syn_ast);
                let librustc_pretty = pprust::expr_to_string(&librustc_ast);
                errorf!(
                    "\nFAIL {}\n{}\nsyn != rustc\n{}\n",
                    path.display(),
                    syn_pretty,
                    librustc_pretty,
                );
                continue;
            }

            let expr_invisible = make_parens_invisible(expr);
            let Ok(reparsed_expr_invisible) = syn::parse2(expr_invisible.to_token_stream()) else {
                failed += 1;
                errorf!(
                    "\nFAIL {} - syn failed to parse invisible delimiters\n{}\n",
                    path.display(),
                    source_code,
                );
                continue;
            };
            if expr_invisible != reparsed_expr_invisible {
                failed += 1;
                errorf!(
                    "\nFAIL {} - mismatch after parsing invisible delimiters\n{}\n",
                    path.display(),
                    source_code,
                );
                continue;
            }

            if scan_expr::scan_expr.parse2(expr_tokens).is_err() {
                failed += 1;
                errorf!(
                    "\nFAIL {} - failed to scan expr\n{}\n",
                    path.display(),
                    source_code,
                );
                continue;
            }

            passed += 1;
        }
    });

    (passed, failed)
}

fn librustc_parse_and_rewrite(input: &str) -> Option<Box<ast::Expr>> {
    parse::librustc_expr(input).map(librustc_parenthesize)
}

fn librustc_parenthesize(mut librustc_expr: Box<ast::Expr>) -> Box<ast::Expr> {
    use rustc_ast::ast::{
        AssocItem, AssocItemKind, Attribute, BinOpKind, Block, BoundConstness, Expr, ExprField,
        ExprKind, GenericArg, GenericBound, Local, LocalKind, Pat, PolyTraitRef, Stmt, StmtKind,
        StructExpr, StructRest, TraitBoundModifiers, Ty,
    };
    use rustc_ast::mut_visit::{walk_flat_map_assoc_item, MutVisitor};
    use rustc_ast::visit::{AssocCtxt, BoundKind};
    use rustc_data_structures::flat_map_in_place::FlatMapInPlace;
    use rustc_span::DUMMY_SP;
    use smallvec::SmallVec;
    use std::ops::DerefMut;
    use thin_vec::ThinVec;

    struct FullyParenthesize;

    fn contains_let_chain(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Let(..) => true,
            ExprKind::Binary(binop, left, right) => {
                binop.node == BinOpKind::And
                    && (contains_let_chain(left) || contains_let_chain(right))
            }
            _ => false,
        }
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
        match &mut e.kind {
            ExprKind::Become(..) => {}
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
            }
            _ => rustc_ast::mut_visit::walk_expr(vis, e),
        }
    }

    impl MutVisitor for FullyParenthesize {
        fn visit_expr(&mut self, e: &mut Expr) {
            noop_visit_expr(e, self);
            match e.kind {
                ExprKind::Block(..) | ExprKind::If(..) | ExprKind::Let(..) => {}
                ExprKind::Binary(..) if contains_let_chain(e) => {}
                _ => {
                    let inner = mem::replace(e, Expr::dummy());
                    *e = Expr {
                        id: ast::DUMMY_NODE_ID,
                        kind: ExprKind::Paren(Box::new(inner)),
                        span: DUMMY_SP,
                        attrs: ThinVec::new(),
                        tokens: None,
                    };
                }
            }
        }

        fn visit_generic_arg(&mut self, arg: &mut GenericArg) {
            match arg {
                GenericArg::Lifetime(_lifetime) => {}
                GenericArg::Type(arg) => self.visit_ty(arg),
                // Don't wrap unbraced const generic arg as that's invalid syntax.
                GenericArg::Const(anon_const) => {
                    if let ExprKind::Block(..) = &mut anon_const.value.kind {
                        noop_visit_expr(&mut anon_const.value, self);
                    }
                }
            }
        }

        fn visit_param_bound(&mut self, bound: &mut GenericBound, _ctxt: BoundKind) {
            match bound {
                GenericBound::Trait(PolyTraitRef {
                    modifiers:
                        TraitBoundModifiers {
                            constness: BoundConstness::Maybe(_),
                            ..
                        },
                    ..
                })
                | GenericBound::Outlives(..)
                | GenericBound::Use(..) => {}
                GenericBound::Trait(ty) => self.visit_poly_trait_ref(ty),
            }
        }

        fn visit_block(&mut self, block: &mut Block) {
            self.visit_id(&mut block.id);
            block
                .stmts
                .flat_map_in_place(|stmt| flat_map_stmt(stmt, self));
            self.visit_span(&mut block.span);
        }

        fn visit_local(&mut self, local: &mut Local) {
            match &mut local.kind {
                LocalKind::Decl => {}
                LocalKind::Init(init) => {
                    self.visit_expr(init);
                }
                LocalKind::InitElse(init, els) => {
                    self.visit_expr(init);
                    self.visit_block(els);
                }
            }
        }

        fn flat_map_assoc_item(
            &mut self,
            item: Box<AssocItem>,
            ctxt: AssocCtxt,
        ) -> SmallVec<[Box<AssocItem>; 1]> {
            match &item.kind {
                AssocItemKind::Const(const_item)
                    if !const_item.generics.params.is_empty()
                        || !const_item.generics.where_clause.predicates.is_empty() =>
                {
                    SmallVec::from([item])
                }
                _ => walk_flat_map_assoc_item(self, item, ctxt),
            }
        }

        // We don't want to look at expressions that might appear in patterns or
        // types yet. We'll look into comparing those in the future. For now
        // focus on expressions appearing in other places.
        fn visit_pat(&mut self, pat: &mut Pat) {
            let _ = pat;
        }

        fn visit_ty(&mut self, ty: &mut Ty) {
            let _ = ty;
        }

        fn visit_attribute(&mut self, attr: &mut Attribute) {
            let _ = attr;
        }
    }

    let mut folder = FullyParenthesize;
    folder.visit_expr(&mut librustc_expr);
    librustc_expr
}

fn syn_parenthesize(syn_expr: syn::Expr) -> syn::Expr {
    use syn::fold::{fold_expr, fold_generic_argument, Fold};
    use syn::{token, BinOp, Expr, ExprParen, GenericArgument, MetaNameValue, Pat, Stmt, Type};

    struct FullyParenthesize;

    fn parenthesize(expr: Expr) -> Expr {
        Expr::Paren(ExprParen {
            attrs: Vec::new(),
            expr: Box::new(expr),
            paren_token: token::Paren::default(),
        })
    }

    fn needs_paren(expr: &Expr) -> bool {
        match expr {
            Expr::Group(_) => unreachable!(),
            Expr::If(_) | Expr::Unsafe(_) | Expr::Block(_) | Expr::Let(_) => false,
            Expr::Binary(_) => !contains_let_chain(expr),
            _ => true,
        }
    }

    fn contains_let_chain(expr: &Expr) -> bool {
        match expr {
            Expr::Let(_) => true,
            Expr::Binary(expr) => {
                matches!(expr.op, BinOp::And(_))
                    && (contains_let_chain(&expr.left) || contains_let_chain(&expr.right))
            }
            _ => false,
        }
    }

    impl Fold for FullyParenthesize {
        fn fold_expr(&mut self, expr: Expr) -> Expr {
            let needs_paren = needs_paren(&expr);
            let folded = fold_expr(self, expr);
            if needs_paren {
                parenthesize(folded)
            } else {
                folded
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

        fn fold_stmt(&mut self, stmt: Stmt) -> Stmt {
            match stmt {
                // Don't wrap toplevel expressions in statements.
                Stmt::Expr(Expr::Verbatim(_), Some(_)) => stmt,
                Stmt::Expr(e, semi) => Stmt::Expr(fold_expr(self, e), semi),
                s => s,
            }
        }

        fn fold_meta_name_value(&mut self, meta: MetaNameValue) -> MetaNameValue {
            // Don't turn #[p = "..."] into #[p = ("...")].
            meta
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

    let mut folder = FullyParenthesize;
    folder.fold_expr(syn_expr)
}

fn make_parens_invisible(expr: syn::Expr) -> syn::Expr {
    use syn::fold::{fold_expr, fold_stmt, Fold};
    use syn::{token, Expr, ExprGroup, ExprParen, Stmt};

    struct MakeParensInvisible;

    impl Fold for MakeParensInvisible {
        fn fold_expr(&mut self, mut expr: Expr) -> Expr {
            if let Expr::Paren(paren) = expr {
                expr = Expr::Group(ExprGroup {
                    attrs: paren.attrs,
                    group_token: token::Group(paren.paren_token.span.join()),
                    expr: paren.expr,
                });
            }
            fold_expr(self, expr)
        }

        fn fold_stmt(&mut self, stmt: Stmt) -> Stmt {
            if let Stmt::Expr(expr @ (Expr::Binary(_) | Expr::Call(_) | Expr::Cast(_)), None) = stmt
            {
                Stmt::Expr(
                    Expr::Paren(ExprParen {
                        attrs: Vec::new(),
                        paren_token: token::Paren::default(),
                        expr: Box::new(fold_expr(self, expr)),
                    }),
                    None,
                )
            } else {
                fold_stmt(self, stmt)
            }
        }
    }

    let mut folder = MakeParensInvisible;
    folder.fold_expr(expr)
}

/// Walk through a crate collecting all expressions we can find in it.
fn collect_exprs(file: syn::File) -> Vec<syn::Expr> {
    use syn::fold::Fold;
    use syn::punctuated::Punctuated;
    use syn::{token, ConstParam, Expr, ExprTuple, Pat, Path};

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

        fn fold_pat(&mut self, pat: Pat) -> Pat {
            pat
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
