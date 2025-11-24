// $ cargo bench --features full,test --bench rust
//
// Syn only, useful for profiling:
// $ RUSTFLAGS='--cfg syn_only' cargo build --release --features full,test --bench rust

#![cfg_attr(not(syn_only), feature(rustc_private))]
#![recursion_limit = "1024"]
#![allow(clippy::cast_lossless, clippy::unnecessary_wraps)]

#[macro_use]
#[path = "../tests/macros/mod.rs"]
mod macros;

#[path = "../tests/common/mod.rs"]
mod common;
#[path = "../tests/repo/mod.rs"]
mod repo;

use std::fs;
use std::time::{Duration, Instant};

#[cfg(not(syn_only))]
mod tokenstream_parse {
    use proc_macro2::TokenStream;
    use std::str::FromStr;

    pub fn bench(content: &str) -> Result<(), ()> {
        TokenStream::from_str(content).map(drop).map_err(drop)
    }
}

mod syn_parse {
    pub fn bench(content: &str) -> Result<(), ()> {
        syn::parse_file(content).map(drop).map_err(drop)
    }
}

#[cfg(not(syn_only))]
mod librustc_parse {
    extern crate rustc_data_structures;
    extern crate rustc_error_messages;
    extern crate rustc_errors;
    extern crate rustc_parse;
    extern crate rustc_session;
    extern crate rustc_span;

    use rustc_data_structures::sync::Lrc;
    use rustc_error_messages::FluentBundle;
    use rustc_errors::{emitter::Emitter, translation::Translate, Diagnostic, Handler};
    use rustc_session::parse::ParseSess;
    use rustc_span::source_map::{FilePathMapping, SourceMap};
    use rustc_span::{edition::Edition, FileName};

    pub fn bench(content: &str) -> Result<(), ()> {
        struct SilentEmitter;

        impl Emitter for SilentEmitter {
            fn emit_diagnostic(&mut self, _diag: &Diagnostic) {}
            fn source_map(&self) -> Option<&Lrc<SourceMap>> {
                None
            }
        }

        impl Translate for SilentEmitter {
            fn fluent_bundle(&self) -> Option<&Lrc<FluentBundle>> {
                None
            }
            fn fallback_fluent_bundle(&self) -> &FluentBundle {
                panic!("silent emitter attempted to translate a diagnostic");
            }
        }

        rustc_span::create_session_if_not_set_then(Edition::Edition2018, |_| {
            let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));
            let emitter = Box::new(SilentEmitter);
            let handler = Handler::with_emitter(false, None, emitter);
            let sess = ParseSess::with_span_handler(handler, cm);
            if let Err(diagnostic) = rustc_parse::parse_crate_from_source_str(
                FileName::Custom("bench".to_owned()),
                content.to_owned(),
                &sess,
            ) {
                diagnostic.cancel();
                return Err(());
            };
            Ok(())
        })
    }
}

#[cfg(not(syn_only))]
mod read_from_disk {
    pub fn bench(content: &str) -> Result<(), ()> {
        _ = content;
        Ok(())
    }
}

fn exec(mut codepath: impl FnMut(&str) -> Result<(), ()>) -> Duration {
    let begin = Instant::now();
    let mut success = 0;
    let mut total = 0;

    walkdir::WalkDir::new("tests/rust/src")
        .into_iter()
        .filter_entry(repo::base_dir_filter)
        .for_each(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                return;
            }
            let content = fs::read_to_string(path).unwrap();
            let ok = codepath(&content).is_ok();
            success += ok as usize;
            total += 1;
            if !ok {
                eprintln!("FAIL {}", path.display());
            }
        });

    assert_eq!(success, total);
    begin.elapsed()
}

fn main() {
    repo::clone_rust();

    macro_rules! testcases {
        ($($(#[$cfg:meta])* $name:ident,)*) => {
            [
                $(
                    $(#[$cfg])*
                    (stringify!($name), $name::bench as fn(&str) -> Result<(), ()>),
                )*
            ]
        };
    }

    #[cfg(not(syn_only))]
    {
        let mut lines = 0;
        let mut files = 0;
        exec(|content| {
            lines += content.lines().count();
            files += 1;
            Ok(())
        });
        eprintln!("\n{} lines in {} files", lines, files);
    }

    for (name, f) in testcases!(
        #[cfg(not(syn_only))]
        read_from_disk,
        #[cfg(not(syn_only))]
        tokenstream_parse,
        syn_parse,
        #[cfg(not(syn_only))]
        librustc_parse,
    ) {
        eprint!("{:20}", format!("{}:", name));
        let elapsed = exec(f);
        eprintln!(
            "elapsed={}.{:03}s",
            elapsed.as_secs(),
            elapsed.subsec_millis(),
        );
    }
    eprintln!();
}
