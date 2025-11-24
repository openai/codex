#![allow(clippy::manual_assert)]

mod progress;

use self::progress::Progress;
use anyhow::Result;
use flate2::read::GzDecoder;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rayon::ThreadPoolBuilder;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;
use walkdir::{DirEntry, WalkDir};

const REVISION: &str = "0aeaa5eb22180fdf12a8489e63c4daa18da6f236";

#[rustfmt::skip]
static EXCLUDE_FILES: &[&str] = &[
    // TODO: non-lifetime binders: `where for<'a, T> &'a Struct<T>: Trait`
    // https://github.com/dtolnay/syn/issues/1435
    "src/tools/rustfmt/tests/source/issue_5721.rs",
    "src/tools/rustfmt/tests/source/non-lifetime-binders.rs",
    "src/tools/rustfmt/tests/target/issue_5721.rs",
    "src/tools/rustfmt/tests/target/non-lifetime-binders.rs",
    "tests/rustdoc-json/non_lifetime_binders.rs",
    "tests/rustdoc/inline_cross/auxiliary/non_lifetime_binders.rs",
    "tests/rustdoc/non_lifetime_binders.rs",

    // TODO: unsafe binders: `unsafe<'a> &'a T`
    // https://github.com/dtolnay/syn/issues/1791
    "src/tools/rustfmt/tests/source/unsafe-binders.rs",
    "src/tools/rustfmt/tests/target/unsafe-binders.rs",

    // TODO: unsafe fields: `struct S { unsafe field: T }`
    // https://github.com/dtolnay/syn/issues/1792
    "src/tools/rustfmt/tests/source/unsafe-field.rs",
    "src/tools/rustfmt/tests/target/unsafe-field.rs",
    "tests/ui/unsafe-fields/auxiliary/unsafe-fields-crate-dep.rs",

    // TODO: guard patterns: `match expr { (A if f()) | (B if g()) => {} }`
    // https://github.com/dtolnay/syn/issues/1793
    "src/tools/rustfmt/tests/target/guard_patterns.rs",

    // TODO: struct field default: `struct S { field: i32 = 1 }`
    // https://github.com/dtolnay/syn/issues/1774
    "tests/ui/structs/auxiliary/struct_field_default.rs",
    "tests/ui/structs/default-field-values-support.rs",

    // TODO: return type notation: `where T: Trait<method(): Send>` and `where T::method(..): Send`
    // https://github.com/dtolnay/syn/issues/1434
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/return_type_syntax_in_path.rs",
    "src/tools/rustfmt/tests/target/return-type-notation.rs",
    "tests/ui/associated-type-bounds/all-generics-lookup.rs",
    "tests/ui/associated-type-bounds/implied-from-self-where-clause.rs",
    "tests/ui/associated-type-bounds/return-type-notation/basic.rs",
    "tests/ui/associated-type-bounds/return-type-notation/higher-ranked-bound-works.rs",
    "tests/ui/associated-type-bounds/return-type-notation/namespace-conflict.rs",
    "tests/ui/associated-type-bounds/return-type-notation/path-constrained-in-method.rs",
    "tests/ui/associated-type-bounds/return-type-notation/path-self-qself.rs",
    "tests/ui/associated-type-bounds/return-type-notation/path-works.rs",
    "tests/ui/associated-type-bounds/return-type-notation/unpretty-parenthesized.rs",
    "tests/ui/async-await/return-type-notation/normalizing-self-auto-trait-issue-109924.rs",
    "tests/ui/async-await/return-type-notation/rtn-implied-in-supertrait.rs",
    "tests/ui/async-await/return-type-notation/supertrait-bound.rs",
    "tests/ui/borrowck/alias-liveness/rtn-static.rs",
    "tests/ui/feature-gates/feature-gate-return_type_notation.rs",

    // TODO: lazy type alias syntax with where-clause in trailing position
    // https://github.com/dtolnay/syn/issues/1525
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/type_item_where_clause.rs",
    "src/tools/rustfmt/tests/source/type-alias-where-clauses-with-comments.rs",
    "src/tools/rustfmt/tests/source/type-alias-where-clauses.rs",
    "src/tools/rustfmt/tests/target/type-alias-where-clauses-with-comments.rs",
    "src/tools/rustfmt/tests/target/type-alias-where-clauses.rs",
    "tests/rustdoc/typedef-inner-variants-lazy_type_alias.rs",

    // TODO: gen blocks and functions
    // https://github.com/dtolnay/syn/issues/1526
    "compiler/rustc_codegen_cranelift/example/gen_block_iterate.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/gen_blocks.rs",
    "tests/ui/coroutine/async-gen-deduce-yield.rs",
    "tests/ui/coroutine/async-gen-yield-ty-is-unit.rs",
    "tests/ui/coroutine/async_gen_fn_iter.rs",
    "tests/ui/coroutine/gen_block_is_fused_iter.rs",
    "tests/ui/coroutine/gen_block_is_iter.rs",
    "tests/ui/coroutine/gen_block_iterate.rs",
    "tests/ui/coroutine/gen_fn_iter.rs",
    "tests/ui/coroutine/gen_fn_lifetime_capture.rs",
    "tests/ui/coroutine/other-attribute-on-gen.rs",
    "tests/ui/coroutine/return-types-diverge.rs",
    "tests/ui/higher-ranked/builtin-closure-like-bounds.rs",
    "tests/ui/sanitizer/cfi/coroutine.rs",

    // TODO: `!` as a pattern
    // https://github.com/dtolnay/syn/issues/1546
    "tests/mir-opt/building/match/never_patterns.rs",
    "tests/ui/rfcs/rfc-0000-never_patterns/diverges.rs",
    "tests/ui/rfcs/rfc-0000-never_patterns/use-bindings.rs",

    // TODO: async trait bounds: `impl async Fn()`
    // https://github.com/dtolnay/syn/issues/1628
    "src/tools/miri/tests/pass/async-closure-captures.rs",
    "src/tools/miri/tests/pass/async-closure-drop.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/async_trait_bound.rs",
    "src/tools/rustfmt/tests/target/asyncness.rs",
    "tests/codegen/async-closure-debug.rs",
    "tests/coverage/async_closure.rs",
    "tests/ui/async-await/async-closures/async-fn-mut-for-async-fn.rs",
    "tests/ui/async-await/async-closures/async-fn-once-for-async-fn.rs",
    "tests/ui/async-await/async-closures/auxiliary/foreign.rs",
    "tests/ui/async-await/async-closures/body-check-on-non-fnmut.rs",
    "tests/ui/async-await/async-closures/box-deref-in-debuginfo.rs",
    "tests/ui/async-await/async-closures/brand.rs",
    "tests/ui/async-await/async-closures/captures.rs",
    "tests/ui/async-await/async-closures/clone-closure.rs",
    "tests/ui/async-await/async-closures/constrained-but-no-upvars-yet.rs",
    "tests/ui/async-await/async-closures/debuginfo-by-move-body.rs",
    "tests/ui/async-await/async-closures/drop.rs",
    "tests/ui/async-await/async-closures/force-move-due-to-inferred-kind.rs",
    "tests/ui/async-await/async-closures/foreign.rs",
    "tests/ui/async-await/async-closures/inline-body.rs",
    "tests/ui/async-await/async-closures/mangle.rs",
    "tests/ui/async-await/async-closures/moro-example.rs",
    "tests/ui/async-await/async-closures/move-is-async-fn.rs",
    "tests/ui/async-await/async-closures/mut-ref-reborrow.rs",
    "tests/ui/async-await/async-closures/no-borrow-from-env.rs",
    "tests/ui/async-await/async-closures/non-copy-arg-does-not-force-inner-move.rs",
    "tests/ui/async-await/async-closures/overlapping-projs.rs",
    "tests/ui/async-await/async-closures/precise-captures.rs",
    "tests/ui/async-await/async-closures/refd.rs",
    "tests/ui/async-await/async-closures/signature-deduction.rs",
    "tests/ui/async-await/async-fn/edition-2015-not-async-bound.rs",
    "tests/ui/async-await/async-fn/higher-ranked-async-fn.rs",
    "tests/ui/async-await/async-fn/impl-trait.rs",
    "tests/ui/async-await/async-fn/project.rs",
    "tests/ui/async-await/async-fn/sugar.rs",

    // TODO: mutable by-reference bindings (mut ref)
    // https://github.com/dtolnay/syn/issues/1629
    "src/tools/rustfmt/tests/source/mut_ref.rs",
    "src/tools/rustfmt/tests/target/mut_ref.rs",
    "tests/ui/mut/mut-ref.rs",

    // TODO: postfix match
    // https://github.com/dtolnay/syn/issues/1630
    "src/tools/rustfmt/tests/source/postfix-match/pf-match.rs",
    "src/tools/rustfmt/tests/target/postfix-match/pf-match.rs",
    "tests/pretty/postfix-match/simple-matches.rs",
    "tests/ui/match/postfix-match/no-unused-parens.rs",
    "tests/ui/match/postfix-match/pf-match-chain.rs",
    "tests/ui/match/postfix-match/postfix-match.rs",

    // TODO: delegation
    // https://github.com/dtolnay/syn/issues/1580
    "tests/pretty/delegation.rs",
    "tests/ui/delegation/body-identity-glob.rs",
    "tests/ui/delegation/body-identity-list.rs",
    "tests/ui/delegation/explicit-paths-in-traits-pass.rs",
    "tests/ui/delegation/explicit-paths-pass.rs",
    "tests/ui/delegation/explicit-paths-signature-pass.rs",
    "tests/ui/delegation/fn-header.rs",
    "tests/ui/delegation/generics/free-fn-to-free-fn-pass.rs",
    "tests/ui/delegation/generics/free-fn-to-trait-method-pass.rs",
    "tests/ui/delegation/generics/impl-to-free-fn-pass.rs",
    "tests/ui/delegation/generics/impl-trait-to-trait-method-pass.rs",
    "tests/ui/delegation/generics/inherent-impl-to-trait-method-pass.rs",
    "tests/ui/delegation/generics/trait-method-to-other-pass.rs",
    "tests/ui/delegation/glob-glob.rs",
    "tests/ui/delegation/glob-override.rs",
    "tests/ui/delegation/glob.rs",
    "tests/ui/delegation/impl-trait.rs",
    "tests/ui/delegation/list.rs",
    "tests/ui/delegation/macro-inside-glob.rs",
    "tests/ui/delegation/macro-inside-list.rs",
    "tests/ui/delegation/method-call-priority.rs",
    "tests/ui/delegation/parse.rs",
    "tests/ui/delegation/rename.rs",
    "tests/ui/delegation/self-coercion.rs",

    // TODO: for await
    // https://github.com/dtolnay/syn/issues/1631
    "tests/ui/async-await/for-await-2015.rs",
    "tests/ui/async-await/for-await-passthrough.rs",
    "tests/ui/async-await/for-await.rs",

    // TODO: const trait bound: `T: const Trait` and `impl const Trait` and `~const Trait`
    // https://github.com/dtolnay/syn/issues/1632
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/const_trait_bound.rs",
    "tests/ui/generic-const-items/const-trait-impl.rs",
    "tests/ui/traits/const-traits/const-bound-in-host.rs",
    "tests/ui/traits/const-traits/const-drop.rs",
    "tests/ui/traits/const-traits/const-impl-trait.rs",
    "tests/ui/traits/const-traits/const-in-closure.rs",
    "tests/ui/traits/const-traits/dont-ice-on-const-pred-for-bounds.rs",
    "tests/ui/traits/const-traits/effects/auxiliary/minicore.rs",
    "tests/ui/traits/const-traits/effects/dont-prefer-param-env-for-infer-self-ty.rs",
    "tests/ui/traits/const-traits/effects/minicore-const-fn-early-bound.rs",
    "tests/ui/traits/const-traits/predicate-entailment-passes.rs",
    "tests/ui/traits/const-traits/tilde-const-syntax.rs",

    // TODO: unparenthesized half-open range pattern inside slice pattern: `[1..]`
    // https://github.com/dtolnay/syn/issues/1769
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/range_pat.rs",
    "tests/ui/consts/miri_unleashed/const_refers_to_static_cross_crate.rs",

    // TODO: pinned type sugar: `&pin const Self`
    // https://github.com/dtolnay/syn/issues/1770
    "src/tools/rustfmt/tests/source/pin_sugar.rs",
    "src/tools/rustfmt/tests/target/pin_sugar.rs",
    "tests/ui/async-await/pin-ergonomics/sugar.rs",

    // TODO: `|| .. .method()`
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/closure_range_method_call.rs",
    "src/tools/rustfmt/tests/source/issue-4808.rs",

    // Several of the above
    "tests/ui/unpretty/expanded-exhaustive.rs",

    // Rustc bug: https://github.com/rust-lang/rust/issues/132080
    "src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0073_safe_declarations_in_extern_blocks.rs",

    // Compile-fail expr parameter in const generic position: f::<1 + 2>()
    "tests/ui/const-generics/early/closing-args-token.rs",
    "tests/ui/const-generics/early/const-expression-parameter.rs",

    // Compile-fail variadics in not the last position of a function parameter list
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/fn_def_param.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/param_list_vararg.rs",
    "tests/ui/parser/variadic-ffi-syntactic-pass.rs",

    // Need at least one trait in impl Trait, no such type as impl 'static
    "tests/ui/type-alias-impl-trait/generic_type_does_not_live_long_enough.rs",

    // Negative polarity trait bound: `where T: !Copy`
    "src/tools/rustfmt/tests/target/negative-bounds.rs",
    "tests/ui/traits/negative-bounds/supertrait.rs",

    // Const impl that is not a trait impl: `impl ~const T {}`
    "tests/ui/traits/const-traits/syntax.rs",

    // Lifetimes and types out of order in angle bracketed path arguments
    "tests/ui/parser/constraints-before-generic-args-syntactic-pass.rs",

    // Deprecated anonymous parameter syntax in traits
    "src/tools/rustfmt/tests/source/trait.rs",
    "src/tools/rustfmt/tests/target/trait.rs",
    "tests/ui/issues/issue-13105.rs",
    "tests/ui/issues/issue-13775.rs",
    "tests/ui/issues/issue-34074.rs",
    "tests/ui/proc-macro/trait-fn-args-2015.rs",

    // Deprecated where-clause location
    "src/tools/rustfmt/tests/source/issue_4257.rs",
    "src/tools/rustfmt/tests/source/issue_4911.rs",
    "src/tools/rustfmt/tests/target/issue_4257.rs",
    "src/tools/rustfmt/tests/target/issue_4911.rs",
    "tests/pretty/gat-bounds.rs",
    "tests/rustdoc/generic-associated-types/gats.rs",

    // Deprecated trait object syntax with parenthesized generic arguments and no dyn keyword
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/path_fn_trait_args.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/typepathfn_with_coloncolon.rs",
    "src/tools/rustfmt/tests/source/attrib.rs",
    "src/tools/rustfmt/tests/source/closure.rs",
    "src/tools/rustfmt/tests/source/existential_type.rs",
    "src/tools/rustfmt/tests/source/fn-simple.rs",
    "src/tools/rustfmt/tests/source/fn_args_layout-vertical.rs",
    "src/tools/rustfmt/tests/source/issue-4689/one.rs",
    "src/tools/rustfmt/tests/source/issue-4689/two.rs",
    "src/tools/rustfmt/tests/source/paths.rs",
    "src/tools/rustfmt/tests/source/structs.rs",
    "src/tools/rustfmt/tests/target/attrib.rs",
    "src/tools/rustfmt/tests/target/closure.rs",
    "src/tools/rustfmt/tests/target/existential_type.rs",
    "src/tools/rustfmt/tests/target/fn-simple.rs",
    "src/tools/rustfmt/tests/target/fn.rs",
    "src/tools/rustfmt/tests/target/fn_args_layout-vertical.rs",
    "src/tools/rustfmt/tests/target/issue-4689/one.rs",
    "src/tools/rustfmt/tests/target/issue-4689/two.rs",
    "src/tools/rustfmt/tests/target/paths.rs",
    "src/tools/rustfmt/tests/target/structs.rs",
    "tests/codegen-units/item-collection/non-generic-closures.rs",
    "tests/debuginfo/recursive-enum.rs",
    "tests/pretty/closure-reform-pretty.rs",
    "tests/run-make/reproducible-build-2/reproducible-build.rs",
    "tests/run-make/reproducible-build/reproducible-build.rs",
    "tests/ui/auxiliary/typeid-intrinsic-aux1.rs",
    "tests/ui/auxiliary/typeid-intrinsic-aux2.rs",
    "tests/ui/impl-trait/generic-with-implicit-hrtb-without-dyn.rs",
    "tests/ui/lifetimes/auxiliary/lifetime_bound_will_change_warning_lib.rs",
    "tests/ui/lifetimes/bare-trait-object-borrowck.rs",
    "tests/ui/lifetimes/bare-trait-object.rs",
    "tests/ui/parser/bounds-obj-parens.rs",

    // Various extensions to Rust syntax made up by rust-analyzer
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/assoc_type_bound.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/const_param_default_path.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/field_expr.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/generic_arg_bounds.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/use_tree_abs_star.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0015_use_tree.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0029_range_forms.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0051_parameter_attrs.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0055_dot_dot_dot.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0068_item_modifiers.rs",
    "src/tools/rust-analyzer/crates/syntax/test_data/parser/validation/0031_block_inner_attrs.rs",
    "src/tools/rust-analyzer/crates/syntax/test_data/parser/validation/0038_endless_inclusive_range.rs",
    "src/tools/rust-analyzer/crates/syntax/test_data/parser/validation/0045_ambiguous_trait_object.rs",
    "src/tools/rust-analyzer/crates/syntax/test_data/parser/validation/0046_mutable_const_item.rs",

    // Placeholder syntax for "throw expressions"
    "compiler/rustc_errors/src/translation.rs",
    "compiler/rustc_expand/src/module.rs",
    "compiler/rustc_trait_selection/src/error_reporting/infer/need_type_info.rs",
    "src/tools/clippy/tests/ui/needless_return.rs",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/yeet_expr.rs",
    "tests/pretty/yeet-expr.rs",
    "tests/ui/try-trait/yeet-for-option.rs",
    "tests/ui/try-trait/yeet-for-result.rs",

    // Edition 2015 code using identifiers that are now keywords
    // TODO: some of these we should probably parse
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/ok/dyn_trait_type_weak.rs",
    "src/tools/rustfmt/tests/source/configs/indent_style/block_call.rs",
    "src/tools/rustfmt/tests/source/configs/use_try_shorthand/false.rs",
    "src/tools/rustfmt/tests/source/configs/use_try_shorthand/true.rs",
    "src/tools/rustfmt/tests/source/issue_1306.rs",
    "src/tools/rustfmt/tests/source/try-conversion.rs",
    "src/tools/rustfmt/tests/target/configs/indent_style/block_call.rs",
    "src/tools/rustfmt/tests/target/configs/use_try_shorthand/false.rs",
    "src/tools/rustfmt/tests/target/issue-1681.rs",
    "src/tools/rustfmt/tests/target/issue_1306.rs",
    "tests/ui/dyn-keyword/dyn-2015-no-warnings-without-lints.rs",
    "tests/ui/editions/edition-keywords-2015-2015.rs",
    "tests/ui/editions/edition-keywords-2015-2018.rs",
    "tests/ui/lint/keyword-idents/auxiliary/multi_file_submod.rs",
    "tests/ui/lint/lint_pre_expansion_extern_module_aux.rs",
    "tests/ui/macros/macro-comma-support-rpass.rs",
    "tests/ui/macros/try-macro.rs",
    "tests/ui/parser/extern-crate-async.rs",
    "tests/ui/try-block/try-is-identifier-edition2015.rs",

    // Excessive nesting
    "tests/ui/issues/issue-74564-if-expr-stack-overflow.rs",

    // Testing tools on invalid syntax
    "src/tools/rustfmt/tests/coverage/target/comments.rs",
    "src/tools/rustfmt/tests/parser/issue-4126/invalid.rs",
    "src/tools/rustfmt/tests/parser/issue_4418.rs",
    "src/tools/rustfmt/tests/parser/stashed-diag.rs",
    "src/tools/rustfmt/tests/parser/stashed-diag2.rs",
    "src/tools/rustfmt/tests/parser/unclosed-delims/issue_4466.rs",
    "src/tools/rustfmt/tests/source/configs/disable_all_formatting/true.rs",
    "src/tools/rustfmt/tests/source/configs/spaces_around_ranges/false.rs",
    "src/tools/rustfmt/tests/source/configs/spaces_around_ranges/true.rs",
    "src/tools/rustfmt/tests/source/type.rs",
    "src/tools/rustfmt/tests/target/configs/spaces_around_ranges/false.rs",
    "src/tools/rustfmt/tests/target/configs/spaces_around_ranges/true.rs",
    "src/tools/rustfmt/tests/target/type.rs",
    "src/tools/rustfmt/tests/target/unsafe_extern_blocks.rs",
    "tests/run-make/translation/test.rs",
    "tests/ui/generics/issue-94432-garbage-ice.rs",

    // Generated file containing a top-level expression, used with `include!`
    "compiler/rustc_codegen_gcc/src/intrinsic/archs.rs",

    // Not actually test cases
    "tests/ui/lint/expansion-time-include.rs",
    "tests/ui/macros/auxiliary/macro-comma-support.rs",
    "tests/ui/macros/auxiliary/macro-include-items-expr.rs",
    "tests/ui/macros/include-single-expr-helper.rs",
    "tests/ui/macros/include-single-expr-helper-1.rs",
    "tests/ui/parser/issues/auxiliary/issue-21146-inc.rs",
];

#[rustfmt::skip]
static EXCLUDE_DIRS: &[&str] = &[
    // Inputs that intentionally do not parse
    "src/tools/rust-analyzer/crates/parser/test_data/parser/err",
    "src/tools/rust-analyzer/crates/parser/test_data/parser/inline/err",

    // Inputs that lex but do not necessarily parse
    "src/tools/rust-analyzer/crates/parser/test_data/lexer",

    // Inputs that used to crash rust-analyzer, but aren't necessarily supposed to parse
    "src/tools/rust-analyzer/crates/syntax/test_data/parser/fuzz-failures",
    "src/tools/rust-analyzer/crates/syntax/test_data/reparse/fuzz-failures",

    // Inputs that crash rustc, making no claim about whether they are valid Rust
    "tests/crashes",
];

// Directories in which a .stderr implies the corresponding .rs is not expected
// to work.
static UI_TEST_DIRS: &[&str] = &["tests/ui", "tests/rustdoc-ui"];

pub fn for_each_rust_file(for_each: impl Fn(&Path) + Sync + Send) {
    let mut rs_files = BTreeSet::new();

    let repo_dir = Path::new("tests/rust");
    for entry in WalkDir::new(repo_dir)
        .into_iter()
        .filter_entry(base_dir_filter)
    {
        let entry = entry.unwrap();
        if !entry.file_type().is_dir() {
            rs_files.insert(entry.into_path());
        }
    }

    for ui_test_dir in UI_TEST_DIRS {
        for entry in WalkDir::new(repo_dir.join(ui_test_dir)) {
            let mut path = entry.unwrap().into_path();
            if path.extension() == Some(OsStr::new("stderr")) {
                loop {
                    rs_files.remove(&path.with_extension("rs"));
                    path = path.with_extension("");
                    if path.extension().is_none() {
                        break;
                    }
                }
            }
        }
    }

    rs_files.par_iter().map(PathBuf::as_path).for_each(for_each);
}

pub fn base_dir_filter(entry: &DirEntry) -> bool {
    let path = entry.path();

    let mut path_string = path.to_string_lossy();
    if cfg!(windows) {
        path_string = path_string.replace('\\', "/").into();
    }
    let path_string = if path_string == "tests/rust" {
        return true;
    } else if let Some(path) = path_string.strip_prefix("tests/rust/") {
        path
    } else {
        panic!("unexpected path in Rust dist: {}", path_string);
    };

    if path.is_dir() {
        return !EXCLUDE_DIRS.contains(&path_string);
    }

    if path.extension() != Some(OsStr::new("rs")) {
        return false;
    }

    !EXCLUDE_FILES.contains(&path_string)
}

#[allow(dead_code)]
pub fn edition(path: &Path) -> &'static str {
    if path.ends_with("dyn-2015-no-warnings-without-lints.rs") {
        "2015"
    } else {
        "2021"
    }
}

#[allow(dead_code)]
pub fn abort_after() -> usize {
    match env::var("ABORT_AFTER_FAILURE") {
        Ok(s) => s.parse().expect("failed to parse ABORT_AFTER_FAILURE"),
        Err(_) => usize::MAX,
    }
}

pub fn rayon_init() {
    let stack_size = match env::var("RUST_MIN_STACK") {
        Ok(s) => s.parse().expect("failed to parse RUST_MIN_STACK"),
        Err(_) => 1024 * 1024 * if cfg!(debug_assertions) { 40 } else { 20 },
    };
    ThreadPoolBuilder::new()
        .stack_size(stack_size)
        .build_global()
        .unwrap();
}

pub fn clone_rust() {
    let needs_clone = match fs::read_to_string("tests/rust/COMMIT") {
        Err(_) => true,
        Ok(contents) => contents.trim() != REVISION,
    };
    if needs_clone {
        download_and_unpack().unwrap();
    }

    let mut missing = String::new();
    let test_src = Path::new("tests/rust");

    let mut exclude_files_set = BTreeSet::new();
    for exclude in EXCLUDE_FILES {
        if !exclude_files_set.insert(exclude) {
            panic!("duplicate path in EXCLUDE_FILES: {}", exclude);
        }
        for dir in EXCLUDE_DIRS {
            if Path::new(exclude).starts_with(dir) {
                panic!("excluded file {} is inside an excluded dir", exclude);
            }
        }
        if !test_src.join(exclude).is_file() {
            missing += "\ntests/rust/";
            missing += exclude;
        }
    }

    let mut exclude_dirs_set = BTreeSet::new();
    for exclude in EXCLUDE_DIRS {
        if !exclude_dirs_set.insert(exclude) {
            panic!("duplicate path in EXCLUDE_DIRS: {}", exclude);
        }
        if !test_src.join(exclude).is_dir() {
            missing += "\ntests/rust/";
            missing += exclude;
            missing += "/";
        }
    }

    if !missing.is_empty() {
        panic!("excluded test file does not exist:{}\n", missing);
    }
}

fn download_and_unpack() -> Result<()> {
    let url = format!(
        "https://github.com/rust-lang/rust/archive/{}.tar.gz",
        REVISION
    );
    let response = reqwest::blocking::get(url)?.error_for_status()?;
    let progress = Progress::new(response);
    let decoder = GzDecoder::new(progress);
    let mut archive = Archive::new(decoder);
    let prefix = format!("rust-{}", REVISION);

    let tests_rust = Path::new("tests/rust");
    if tests_rust.exists() {
        fs::remove_dir_all(tests_rust)?;
    }

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path == Path::new("pax_global_header") {
            continue;
        }
        let relative = path.strip_prefix(&prefix)?;
        let out = tests_rust.join(relative);
        entry.unpack(&out)?;
    }

    fs::write("tests/rust/COMMIT", REVISION)?;
    Ok(())
}
