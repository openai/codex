use proc_macro2::{Ident, Span, TokenStream};
use std::str::FromStr;
use syn::Result;

fn parse(s: &str) -> Result<Ident> {
    syn::parse2(TokenStream::from_str(s).unwrap())
}

fn new(s: &str) -> Ident {
    Ident::new(s, Span::call_site())
}

#[test]
fn ident_parse() {
    parse("String").unwrap();
}

#[test]
fn ident_parse_keyword() {
    parse("abstract").unwrap_err();
}

#[test]
fn ident_parse_empty() {
    parse("").unwrap_err();
}

#[test]
fn ident_parse_lifetime() {
    parse("'static").unwrap_err();
}

#[test]
fn ident_parse_underscore() {
    parse("_").unwrap_err();
}

#[test]
fn ident_parse_number() {
    parse("255").unwrap_err();
}

#[test]
fn ident_parse_invalid() {
    parse("a#").unwrap_err();
}

#[test]
fn ident_new() {
    new("String");
}

#[test]
fn ident_new_keyword() {
    new("abstract");
}

#[test]
#[should_panic(expected = "use Option<Ident>")]
fn ident_new_empty() {
    new("");
}

#[test]
#[should_panic(expected = "not a valid Ident")]
fn ident_new_lifetime() {
    new("'static");
}

#[test]
fn ident_new_underscore() {
    new("_");
}

#[test]
#[should_panic(expected = "use Literal instead")]
fn ident_new_number() {
    new("255");
}

#[test]
#[should_panic(expected = "\"a#\" is not a valid Ident")]
fn ident_new_invalid() {
    new("a#");
}
