#![feature(rustc_private)]

mod comment_parser;

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::fn_def_id;
use clippy_utils::is_res_lang_ctor;
use clippy_utils::peel_blocks;
use clippy_utils::source::snippet;
use rustc_ast::LitKind;
use rustc_errors::Applicability;
use rustc_hir::Expr;
use rustc_hir::ExprKind;
use rustc_hir::LangItem;
use rustc_hir::UnOp;
use rustc_hir::def::DefKind;
use rustc_lint::LateContext;
use rustc_lint::LateLintPass;
use rustc_span::BytePos;
use rustc_span::Span;

use crate::comment_parser::parse_argument_comment;
use crate::comment_parser::parse_argument_comment_prefix;

#[cfg(not(feature = "bazel_native"))]
dylint_linting::dylint_library!();

#[unsafe(no_mangle)]
pub fn register_lints(_sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    lint_store.register_lints(&[
        ARGUMENT_COMMENT_MISMATCH,
        UNCOMMENTED_ANONYMOUS_LITERAL_ARGUMENT,
    ]);
    lint_store.register_late_pass(|_| Box::new(ArgumentCommentLint));
}

rustc_session::declare_lint! {
    /// ### What it does
    ///
    /// Checks `/*param*/` argument comments and verifies that the comment
    /// matches the resolved callee parameter name.
    ///
    /// ### Why is this bad?
    ///
    /// A mismatched comment is worse than no comment because it actively
    /// misleads the reader.
    ///
    /// ### Known problems
    ///
    /// This lint only runs when the callee resolves to a concrete function or
    /// method with available parameter names.
    ///
    /// ### Example
    ///
    /// ```rust
    /// fn create_openai_url(base_url: Option<String>) -> String {
    ///     String::new()
    /// }
    ///
    /// create_openai_url(/*api_base*/ None);
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    /// fn create_openai_url(base_url: Option<String>) -> String {
    ///     String::new()
    /// }
    ///
    /// create_openai_url(/*base_url*/ None);
    /// ```
    pub ARGUMENT_COMMENT_MISMATCH,
    Warn,
    "argument comment does not match the resolved parameter name"
}

rustc_session::declare_lint! {
    /// ### What it does
    ///
    /// Requires a `/*param*/` comment before anonymous literal-like
    /// arguments such as `None`, booleans, and numeric literals.
    ///
    /// ### Why is this bad?
    ///
    /// Bare literal-like arguments make call sites harder to read because the
    /// meaning of the value is hidden in the callee signature.
    ///
    /// ### Known problems
    ///
    /// This lint is opinionated, so it is `allow` by default.
    ///
    /// ### Example
    ///
    /// ```rust
    /// fn create_openai_url(base_url: Option<String>) -> String {
    ///     String::new()
    /// }
    ///
    /// create_openai_url(None);
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    /// fn create_openai_url(base_url: Option<String>) -> String {
    ///     String::new()
    /// }
    ///
    /// create_openai_url(/*base_url*/ None);
    /// ```
    pub UNCOMMENTED_ANONYMOUS_LITERAL_ARGUMENT,
    Allow,
    "anonymous literal-like argument is missing a `/*param*/` comment"
}

#[derive(Default)]
pub struct ArgumentCommentLint;

rustc_session::impl_lint_pass!(
    ArgumentCommentLint => [ARGUMENT_COMMENT_MISMATCH, UNCOMMENTED_ANONYMOUS_LITERAL_ARGUMENT]
);

impl<'tcx> LateLintPass<'tcx> for ArgumentCommentLint {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        match expr.kind {
            ExprKind::Call(callee, args) => {
                self.check_call(cx, expr, callee.span, args, 0);
            }
            ExprKind::MethodCall(_, receiver, args, _) => {
                self.check_call(cx, expr, receiver.span, args, 1);
            }
            _ => {}
        }
    }
}

impl ArgumentCommentLint {
    fn check_call<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        call: &'tcx Expr<'tcx>,
        first_gap_anchor: Span,
        args: &'tcx [Expr<'tcx>],
        parameter_offset: usize,
    ) {
        let Some(def_id) = fn_def_id(cx, call) else {
            return;
        };
        if !def_id.is_local() && !is_workspace_crate_name(cx.tcx.crate_name(def_id.krate).as_str())
        {
            return;
        }
        if !matches!(cx.tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn) {
            return;
        }

        let parameter_names: Vec<_> = cx.tcx.fn_arg_idents(def_id).iter().copied().collect();
        for (index, arg) in args.iter().enumerate() {
            if arg.span.from_expansion() {
                continue;
            }

            let Some(expected_name) = parameter_names.get(index + parameter_offset) else {
                continue;
            };
            let Some(expected_name) = expected_name else {
                continue;
            };
            let expected_name = expected_name.name.to_string();
            if !is_meaningful_parameter_name(&expected_name) {
                continue;
            }

            let boundary_span = if index == 0 {
                first_gap_anchor
            } else {
                args[index - 1].span
            };
            let gap_span = boundary_span.between(arg.span);
            let gap_text = snippet(cx, gap_span, "");
            let arg_text = snippet(cx, arg.span, "..");
            let lookbehind_start = BytePos(arg.span.lo().0.saturating_sub(64));
            let lookbehind_text =
                snippet(cx, arg.span.shrink_to_lo().with_lo(lookbehind_start), "");
            let argument_comment = parse_argument_comment(gap_text.as_ref())
                .or_else(|| parse_argument_comment(lookbehind_text.as_ref()))
                .or_else(|| parse_argument_comment_prefix(arg_text.as_ref()));

            if let Some(actual_name) = argument_comment {
                if actual_name != expected_name {
                    span_lint_and_help(
                        cx,
                        ARGUMENT_COMMENT_MISMATCH,
                        arg.span,
                        format!(
                            "argument comment `/*{actual_name}*/` does not match parameter `{expected_name}`"
                        ),
                        None,
                        format!("use `/*{expected_name}*/`"),
                    );
                }
                continue;
            }

            if !is_anonymous_literal_like(cx, arg) {
                continue;
            }

            span_lint_and_sugg(
                cx,
                UNCOMMENTED_ANONYMOUS_LITERAL_ARGUMENT,
                arg.span,
                format!("anonymous literal-like argument for parameter `{expected_name}`"),
                "prepend the parameter name comment",
                format!("/*{expected_name}*/ {arg_text}"),
                Applicability::MachineApplicable,
            );
        }
    }
}

fn is_anonymous_literal_like(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let expr = peel_blocks(expr);
    match expr.kind {
        ExprKind::Lit(lit) => !matches!(
            lit.node,
            LitKind::Str(..) | LitKind::ByteStr(..) | LitKind::CStr(..) | LitKind::Char(..)
        ),
        ExprKind::Unary(UnOp::Neg, inner) => matches!(peel_blocks(inner).kind, ExprKind::Lit(_)),
        ExprKind::Path(qpath) => {
            is_res_lang_ctor(cx, cx.qpath_res(&qpath, expr.hir_id), LangItem::OptionNone)
        }
        _ => false,
    }
}

fn is_meaningful_parameter_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('_')
}

fn is_workspace_crate_name(name: &str) -> bool {
    name.starts_with("codex_")
        || matches!(
            name,
            "app_test_support" | "core_test_support" | "mcp_test_support"
        )
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn workspace_crate_filter_accepts_first_party_names_only() {
    assert!(is_workspace_crate_name("codex_core"));
    assert!(is_workspace_crate_name("codex_tui"));
    assert!(is_workspace_crate_name("core_test_support"));
    assert!(!is_workspace_crate_name("std"));
    assert!(!is_workspace_crate_name("tokio"));
}
