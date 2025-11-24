#![cfg(not(syn_disable_nightly_tests))]
#![cfg(not(miri))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]
#![allow(
    clippy::blocks_in_conditions,
    clippy::elidable_lifetime_names,
    clippy::manual_assert,
    clippy::manual_let_else,
    clippy::match_like_matches_macro,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args
)]
#![allow(mismatched_lifetime_syntaxes)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_error_messages;
extern crate rustc_errors;
extern crate rustc_expand;
extern crate rustc_parse as parse;
extern crate rustc_session;
extern crate rustc_span;

use crate::common::eq::SpanlessEq;
use quote::quote;
use rustc_ast::ast::{
    AngleBracketedArg, Crate, GenericArg, GenericArgs, GenericParamKind, Generics,
};
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast_pretty::pprust;
use rustc_data_structures::flat_map_in_place::FlatMapInPlace;
use rustc_error_messages::{DiagMessage, LazyFallbackBundle};
use rustc_errors::{translation, Diag, PResult};
use rustc_session::parse::ParseSess;
use rustc_span::FileName;
use std::borrow::Cow;
use std::fs;
use std::panic;
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[macro_use]
mod macros;

mod common;
mod repo;

#[test]
fn test_round_trip() {
    repo::rayon_init();
    repo::clone_rust();
    let abort_after = repo::abort_after();
    if abort_after == 0 {
        panic!("skipping all round_trip tests");
    }

    let failed = AtomicUsize::new(0);

    repo::for_each_rust_file(|path| test(path, &failed, abort_after));

    let failed = failed.into_inner();
    if failed > 0 {
        panic!("{} failures", failed);
    }
}

fn test(path: &Path, failed: &AtomicUsize, abort_after: usize) {
    let failed = || {
        let prev_failed = failed.fetch_add(1, Ordering::Relaxed);
        if prev_failed + 1 >= abort_after {
            process::exit(1);
        }
    };

    let content = fs::read_to_string(path).unwrap();

    let (back, elapsed) = match panic::catch_unwind(|| {
        let start = Instant::now();
        let result = syn::parse_file(&content);
        let elapsed = start.elapsed();
        result.map(|krate| (quote!(#krate).to_string(), elapsed))
    }) {
        Err(_) => {
            errorf!("=== {}: syn panic\n", path.display());
            failed();
            return;
        }
        Ok(Err(msg)) => {
            errorf!("=== {}: syn failed to parse\n{:?}\n", path.display(), msg);
            failed();
            return;
        }
        Ok(Ok(result)) => result,
    };

    let edition = repo::edition(path).parse().unwrap();

    rustc_span::create_session_if_not_set_then(edition, |_| {
        let equal = match panic::catch_unwind(|| {
            let locale_resources = rustc_driver::DEFAULT_LOCALE_RESOURCES.to_vec();
            let sess = ParseSess::new(locale_resources);
            let before = match librustc_parse(content, &sess) {
                Ok(before) => before,
                Err(diagnostic) => {
                    errorf!(
                        "=== {}: ignore - librustc failed to parse original content: {}\n",
                        path.display(),
                        translate_message(&diagnostic),
                    );
                    diagnostic.cancel();
                    return Err(true);
                }
            };
            let after = match librustc_parse(back, &sess) {
                Ok(after) => after,
                Err(diagnostic) => {
                    errorf!("=== {}: librustc failed to parse", path.display());
                    diagnostic.emit();
                    return Err(false);
                }
            };
            Ok((before, after))
        }) {
            Err(_) => {
                errorf!("=== {}: ignoring librustc panic\n", path.display());
                true
            }
            Ok(Err(equal)) => equal,
            Ok(Ok((mut before, mut after))) => {
                normalize(&mut before);
                normalize(&mut after);
                if SpanlessEq::eq(&before, &after) {
                    errorf!(
                        "=== {}: pass in {}ms\n",
                        path.display(),
                        elapsed.as_secs() * 1000 + u64::from(elapsed.subsec_nanos()) / 1_000_000
                    );
                    true
                } else {
                    errorf!(
                        "=== {}: FAIL\n{}\n!=\n{}\n",
                        path.display(),
                        pprust::crate_to_string_for_macros(&before),
                        pprust::crate_to_string_for_macros(&after),
                    );
                    false
                }
            }
        };
        if !equal {
            failed();
        }
    });
}

fn librustc_parse(content: String, sess: &ParseSess) -> PResult<Crate> {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = FileName::Custom(format!("test_round_trip{}", counter));
    let mut parser = parse::new_parser_from_source_str(sess, name, content).unwrap();
    parser.parse_crate_mod()
}

fn translate_message(diagnostic: &Diag) -> Cow<'static, str> {
    thread_local! {
        static FLUENT_BUNDLE: LazyFallbackBundle = {
            let locale_resources = rustc_driver::DEFAULT_LOCALE_RESOURCES.to_vec();
            let with_directionality_markers = false;
            rustc_error_messages::fallback_fluent_bundle(locale_resources, with_directionality_markers)
        };
    }

    let message = &diagnostic.messages[0].0;
    let args = translation::to_fluent_args(diagnostic.args.iter());

    let (identifier, attr) = match message {
        DiagMessage::Str(msg) | DiagMessage::Translated(msg) => return msg.clone(),
        DiagMessage::FluentIdentifier(identifier, attr) => (identifier, attr),
    };

    FLUENT_BUNDLE.with(|fluent_bundle| {
        let message = fluent_bundle
            .get_message(identifier)
            .expect("missing diagnostic in fluent bundle");
        let value = match attr {
            Some(attr) => message
                .get_attribute(attr)
                .expect("missing attribute in fluent message")
                .value(),
            None => message.value().expect("missing value in fluent message"),
        };

        let mut err = Vec::new();
        let translated = fluent_bundle.format_pattern(value, Some(&args), &mut err);
        assert!(err.is_empty());
        Cow::Owned(translated.into_owned())
    })
}

fn normalize(krate: &mut Crate) {
    struct NormalizeVisitor;

    impl MutVisitor for NormalizeVisitor {
        fn visit_generic_args(&mut self, e: &mut GenericArgs) {
            if let GenericArgs::AngleBracketed(e) = e {
                #[derive(Ord, PartialOrd, Eq, PartialEq)]
                enum Group {
                    Lifetimes,
                    TypesAndConsts,
                    Constraints,
                }
                e.args.sort_by_key(|arg| match arg {
                    AngleBracketedArg::Arg(arg) => match arg {
                        GenericArg::Lifetime(_) => Group::Lifetimes,
                        GenericArg::Type(_) | GenericArg::Const(_) => Group::TypesAndConsts,
                    },
                    AngleBracketedArg::Constraint(_) => Group::Constraints,
                });
            }
            mut_visit::walk_generic_args(self, e);
        }

        fn visit_generics(&mut self, e: &mut Generics) {
            #[derive(Ord, PartialOrd, Eq, PartialEq)]
            enum Group {
                Lifetimes,
                TypesAndConsts,
            }
            e.params.sort_by_key(|param| match param.kind {
                GenericParamKind::Lifetime => Group::Lifetimes,
                GenericParamKind::Type { .. } | GenericParamKind::Const { .. } => {
                    Group::TypesAndConsts
                }
            });
            e.params
                .flat_map_in_place(|param| self.flat_map_generic_param(param));
            if e.where_clause.predicates.is_empty() {
                e.where_clause.has_where_token = false;
            }
        }
    }

    NormalizeVisitor.visit_crate(krate);
}
