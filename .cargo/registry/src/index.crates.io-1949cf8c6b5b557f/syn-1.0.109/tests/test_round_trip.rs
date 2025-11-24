#![cfg(not(syn_disable_nightly_tests))]
#![cfg(not(miri))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]
#![allow(clippy::manual_assert)]

extern crate rustc_ast;
extern crate rustc_data_structures;
extern crate rustc_error_messages;
extern crate rustc_errors;
extern crate rustc_expand;
extern crate rustc_parse as parse;
extern crate rustc_session;
extern crate rustc_span;

use crate::common::eq::SpanlessEq;
use quote::quote;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rustc_ast::ast::{
    AngleBracketedArg, AngleBracketedArgs, Crate, GenericArg, GenericParamKind, Generics,
    WhereClause,
};
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_error_messages::{DiagnosticMessage, LazyFallbackBundle};
use rustc_errors::{translation, Diagnostic, PResult};
use rustc_session::parse::ParseSess;
use rustc_span::source_map::FilePathMapping;
use rustc_span::FileName;
use std::fs;
use std::panic;
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use walkdir::{DirEntry, WalkDir};

#[macro_use]
mod macros;

#[allow(dead_code)]
mod common;

mod repo;

#[test]
fn test_round_trip() {
    common::rayon_init();
    repo::clone_rust();
    let abort_after = common::abort_after();
    if abort_after == 0 {
        panic!("Skipping all round_trip tests");
    }

    let failed = AtomicUsize::new(0);

    WalkDir::new("tests/rust")
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter()
        .filter_entry(repo::base_dir_filter)
        .collect::<Result<Vec<DirEntry>, walkdir::Error>>()
        .unwrap()
        .into_par_iter()
        .for_each(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                test(path, &failed, abort_after);
            }
        });

    let failed = failed.load(Ordering::Relaxed);
    if failed > 0 {
        panic!("{} failures", failed);
    }
}

fn test(path: &Path, failed: &AtomicUsize, abort_after: usize) {
    let content = fs::read_to_string(path).unwrap();

    let start = Instant::now();
    let (krate, elapsed) = match syn::parse_file(&content) {
        Ok(krate) => (krate, start.elapsed()),
        Err(msg) => {
            errorf!("=== {}: syn failed to parse\n{:?}\n", path.display(), msg);
            let prev_failed = failed.fetch_add(1, Ordering::Relaxed);
            if prev_failed + 1 >= abort_after {
                process::exit(1);
            }
            return;
        }
    };
    let back = quote!(#krate).to_string();
    let edition = repo::edition(path).parse().unwrap();

    rustc_span::create_session_if_not_set_then(edition, |_| {
        let equal = match panic::catch_unwind(|| {
            let sess = ParseSess::new(FilePathMapping::empty());
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
                Err(mut diagnostic) => {
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
                        "=== {}: FAIL\nbefore: {:#?}\nafter: {:#?}\n",
                        path.display(),
                        before,
                        after,
                    );
                    false
                }
            }
        };
        if !equal {
            let prev_failed = failed.fetch_add(1, Ordering::Relaxed);
            if prev_failed + 1 >= abort_after {
                process::exit(1);
            }
        }
    });
}

fn librustc_parse(content: String, sess: &ParseSess) -> PResult<Crate> {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = FileName::Custom(format!("test_round_trip{}", counter));
    parse::parse_crate_from_source_str(name, content, sess)
}

fn translate_message(diagnostic: &Diagnostic) -> String {
    thread_local! {
        static FLUENT_BUNDLE: LazyFallbackBundle = {
            let resources = rustc_error_messages::DEFAULT_LOCALE_RESOURCES;
            let with_directionality_markers = false;
            rustc_error_messages::fallback_fluent_bundle(resources, with_directionality_markers)
        };
    }

    let message = &diagnostic.message[0].0;
    let args = translation::to_fluent_args(diagnostic.args());

    let (identifier, attr) = match message {
        DiagnosticMessage::Str(msg) | DiagnosticMessage::Eager(msg) => return msg.clone(),
        DiagnosticMessage::FluentIdentifier(identifier, attr) => (identifier, attr),
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
        translated.into_owned()
    })
}

fn normalize(krate: &mut Crate) {
    struct NormalizeVisitor;

    impl MutVisitor for NormalizeVisitor {
        fn visit_angle_bracketed_parameter_data(&mut self, e: &mut AngleBracketedArgs) {
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
            mut_visit::noop_visit_angle_bracketed_parameter_data(e, self);
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
            mut_visit::noop_visit_generics(e, self);
        }

        fn visit_where_clause(&mut self, e: &mut WhereClause) {
            if e.predicates.is_empty() {
                e.has_where_token = false;
            }
        }
    }

    NormalizeVisitor.visit_crate(krate);
}
