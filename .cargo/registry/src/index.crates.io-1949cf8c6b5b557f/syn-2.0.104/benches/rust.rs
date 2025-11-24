// $ cargo bench --features full,test --bench rust
//
// Syn only, useful for profiling:
// $ RUSTFLAGS='--cfg syn_only' cargo build --release --features full,test --bench rust

#![cfg_attr(not(syn_only), feature(rustc_private))]
#![recursion_limit = "1024"]
#![allow(
    clippy::arc_with_non_send_sync,
    clippy::cast_lossless,
    clippy::elidable_lifetime_names,
    clippy::let_underscore_untyped,
    clippy::manual_let_else,
    clippy::match_like_matches_macro,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args,
    clippy::unnecessary_wraps
)]

#[macro_use]
#[path = "../tests/macros/mod.rs"]
mod macros;

#[allow(dead_code)]
#[path = "../tests/repo/mod.rs"]
mod repo;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(not(syn_only))]
mod tokenstream_parse {
    use proc_macro2::TokenStream;
    use std::path::Path;
    use std::str::FromStr;

    pub fn bench(_path: &Path, content: &str) -> Result<(), ()> {
        TokenStream::from_str(content).map(drop).map_err(drop)
    }
}

mod syn_parse {
    use std::path::Path;

    pub fn bench(_path: &Path, content: &str) -> Result<(), ()> {
        syn::parse_file(content).map(drop).map_err(drop)
    }
}

#[cfg(not(syn_only))]
mod librustc_parse {
    extern crate rustc_data_structures;
    extern crate rustc_driver;
    extern crate rustc_error_messages;
    extern crate rustc_errors;
    extern crate rustc_parse;
    extern crate rustc_session;
    extern crate rustc_span;

    use crate::repo;
    use rustc_error_messages::FluentBundle;
    use rustc_errors::emitter::Emitter;
    use rustc_errors::registry::Registry;
    use rustc_errors::translation::Translate;
    use rustc_errors::{DiagCtxt, DiagInner};
    use rustc_session::parse::ParseSess;
    use rustc_span::source_map::{FilePathMapping, SourceMap};
    use rustc_span::FileName;
    use std::path::Path;
    use std::sync::Arc;

    pub fn bench(path: &Path, content: &str) -> Result<(), ()> {
        struct SilentEmitter;

        impl Emitter for SilentEmitter {
            fn emit_diagnostic(&mut self, _diag: DiagInner, _registry: &Registry) {}
            fn source_map(&self) -> Option<&SourceMap> {
                None
            }
        }

        impl Translate for SilentEmitter {
            fn fluent_bundle(&self) -> Option<&FluentBundle> {
                None
            }
            fn fallback_fluent_bundle(&self) -> &FluentBundle {
                panic!("silent emitter attempted to translate a diagnostic");
            }
        }

        let edition = repo::edition(path).parse().unwrap();
        rustc_span::create_session_if_not_set_then(edition, |_| {
            let source_map = Arc::new(SourceMap::new(FilePathMapping::empty()));
            let emitter = Box::new(SilentEmitter);
            let handler = DiagCtxt::new(emitter);
            let sess = ParseSess::with_dcx(handler, source_map);
            let name = FileName::Custom("bench".to_owned());
            let mut parser =
                rustc_parse::new_parser_from_source_str(&sess, name, content.to_owned()).unwrap();
            if let Err(diagnostic) = parser.parse_crate_mod() {
                diagnostic.cancel();
                return Err(());
            }
            Ok(())
        })
    }
}

#[cfg(not(syn_only))]
mod read_from_disk {
    use std::path::Path;

    pub fn bench(_path: &Path, content: &str) -> Result<(), ()> {
        let _ = content;
        Ok(())
    }
}

fn exec(mut codepath: impl FnMut(&Path, &str) -> Result<(), ()>) -> Duration {
    let begin = Instant::now();
    let mut success = 0;
    let mut total = 0;

    ["tests/rust/compiler", "tests/rust/library"]
        .iter()
        .flat_map(|dir| {
            walkdir::WalkDir::new(dir)
                .into_iter()
                .filter_entry(repo::base_dir_filter)
        })
        .for_each(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                return;
            }
            let content = fs::read_to_string(path).unwrap();
            let ok = codepath(path, &content).is_ok();
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
                    (stringify!($name), $name::bench as fn(&Path, &str) -> Result<(), ()>),
                )*
            ]
        };
    }

    #[cfg(not(syn_only))]
    {
        let mut lines = 0;
        let mut files = 0;
        exec(|_path, content| {
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
