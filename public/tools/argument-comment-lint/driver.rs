#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;

use std::env;
use std::ffi::OsString;
use std::path::Path;

fn main() {
    let mut callbacks = Callbacks;
    let args = rustc_args(env::args_os().skip(1).collect());
    rustc_driver::run_compiler(&args, &mut callbacks);
}

struct Callbacks;

impl rustc_driver::Callbacks for Callbacks {
    fn config(&mut self, config: &mut rustc_interface::Config) {
        let previous = config.register_lints.take();
        config.register_lints = Some(Box::new(move |sess, lint_store| {
            if let Some(previous) = &previous {
                previous(sess, lint_store);
            }
            argument_comment_lint::register_lints(sess, lint_store);
        }));
    }
}

fn rustc_args(args: Vec<OsString>) -> Vec<String> {
    let mut rustc_args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    if rustc_args.first().is_none_or(|arg| !is_rustc(arg)) {
        rustc_args.insert(0, "rustc".to_string());
    }
    rustc_args
}

fn is_rustc(arg: &str) -> bool {
    Path::new(arg).file_stem().and_then(|stem| stem.to_str()) == Some("rustc")
}
