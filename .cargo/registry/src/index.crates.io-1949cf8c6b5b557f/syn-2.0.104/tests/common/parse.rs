extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_expand;
extern crate rustc_parse as parse;
extern crate rustc_session;
extern crate rustc_span;

use rustc_ast::ast;
use rustc_session::parse::ParseSess;
use rustc_span::FileName;
use std::panic;

pub fn librustc_expr(input: &str) -> Option<Box<ast::Expr>> {
    match panic::catch_unwind(|| {
        let locale_resources = rustc_driver::DEFAULT_LOCALE_RESOURCES.to_vec();
        let sess = ParseSess::new(locale_resources);
        let name = FileName::Custom("test_precedence".to_string());
        let mut parser = parse::new_parser_from_source_str(&sess, name, input.to_string()).unwrap();
        let presult = parser.parse_expr();
        match presult {
            Ok(expr) => Some(expr),
            Err(diagnostic) => {
                diagnostic.emit();
                None
            }
        }
    }) {
        Ok(Some(e)) => Some(e),
        Ok(None) => None,
        Err(_) => {
            errorf!("librustc panicked\n");
            None
        }
    }
}

pub fn syn_expr(input: &str) -> Option<syn::Expr> {
    match syn::parse_str(input) {
        Ok(e) => Some(e),
        Err(msg) => {
            errorf!("syn failed to parse\n{:?}\n", msg);
            None
        }
    }
}
