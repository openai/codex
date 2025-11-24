#![allow(
    clippy::missing_panics_doc,
    clippy::shadow_unrelated,
    clippy::toplevel_ref_arg,
    clippy::uninlined_format_args,
    clippy::wildcard_imports
)]

mod node;
mod util;

use crate::util::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[cfg(test_node_semver)]
use node::{req, VersionReq};
#[cfg(not(test_node_semver))]
use semver::VersionReq;

#[track_caller]
fn assert_match_all(req: &VersionReq, versions: &[&str]) {
    for string in versions {
        let parsed = version(string);
        assert!(req.matches(&parsed), "did not match {}", string);
    }
}

#[track_caller]
fn assert_match_none(req: &VersionReq, versions: &[&str]) {
    for string in versions {
        let parsed = version(string);
        assert!(!req.matches(&parsed), "matched {}", string);
    }
}

#[test]
fn test_basic() {
    let ref r = req("1.0.0");
    assert_to_string(r, "^1.0.0");
    assert_match_all(r, &["1.0.0", "1.1.0", "1.0.1"]);
    assert_match_none(r, &["0.9.9", "0.10.0", "0.1.0", "1.0.0-pre", "1.0.1-pre"]);
}

#[test]
fn test_default() {
    let ref r = VersionReq::default();
    assert_eq!(r, &VersionReq::STAR);
}

#[test]
fn test_exact() {
    let ref r = req("=1.0.0");
    assert_to_string(r, "=1.0.0");
    assert_match_all(r, &["1.0.0"]);
    assert_match_none(r, &["1.0.1", "0.9.9", "0.10.0", "0.1.0", "1.0.0-pre"]);

    let ref r = req("=0.9.0");
    assert_to_string(r, "=0.9.0");
    assert_match_all(r, &["0.9.0"]);
    assert_match_none(r, &["0.9.1", "1.9.0", "0.0.9", "0.9.0-pre"]);

    let ref r = req("=0.0.2");
    assert_to_string(r, "=0.0.2");
    assert_match_all(r, &["0.0.2"]);
    assert_match_none(r, &["0.0.1", "0.0.3", "0.0.2-pre"]);

    let ref r = req("=0.1.0-beta2.a");
    assert_to_string(r, "=0.1.0-beta2.a");
    assert_match_all(r, &["0.1.0-beta2.a"]);
    assert_match_none(r, &["0.9.1", "0.1.0", "0.1.1-beta2.a", "0.1.0-beta2"]);

    let ref r = req("=0.1.0+meta");
    assert_to_string(r, "=0.1.0");
    assert_match_all(r, &["0.1.0", "0.1.0+meta", "0.1.0+any"]);
}

#[test]
pub fn test_greater_than() {
    let ref r = req(">= 1.0.0");
    assert_to_string(r, ">=1.0.0");
    assert_match_all(r, &["1.0.0", "2.0.0"]);
    assert_match_none(r, &["0.1.0", "0.0.1", "1.0.0-pre", "2.0.0-pre"]);

    let ref r = req(">= 2.1.0-alpha2");
    assert_to_string(r, ">=2.1.0-alpha2");
    assert_match_all(r, &["2.1.0-alpha2", "2.1.0-alpha3", "2.1.0", "3.0.0"]);
    assert_match_none(
        r,
        &["2.0.0", "2.1.0-alpha1", "2.0.0-alpha2", "3.0.0-alpha2"],
    );
}

#[test]
pub fn test_less_than() {
    let ref r = req("< 1.0.0");
    assert_to_string(r, "<1.0.0");
    assert_match_all(r, &["0.1.0", "0.0.1"]);
    assert_match_none(r, &["1.0.0", "1.0.0-beta", "1.0.1", "0.9.9-alpha"]);

    let ref r = req("<= 2.1.0-alpha2");
    assert_match_all(r, &["2.1.0-alpha2", "2.1.0-alpha1", "2.0.0", "1.0.0"]);
    assert_match_none(
        r,
        &["2.1.0", "2.2.0-alpha1", "2.0.0-alpha2", "1.0.0-alpha2"],
    );

    let ref r = req(">1.0.0-alpha, <1.0.0");
    assert_match_all(r, &["1.0.0-beta"]);

    let ref r = req(">1.0.0-alpha, <1.0");
    assert_match_none(r, &["1.0.0-beta"]);

    let ref r = req(">1.0.0-alpha, <1");
    assert_match_none(r, &["1.0.0-beta"]);
}

#[test]
pub fn test_multiple() {
    let ref r = req("> 0.0.9, <= 2.5.3");
    assert_to_string(r, ">0.0.9, <=2.5.3");
    assert_match_all(r, &["0.0.10", "1.0.0", "2.5.3"]);
    assert_match_none(r, &["0.0.8", "2.5.4"]);

    let ref r = req("0.3.0, 0.4.0");
    assert_to_string(r, "^0.3.0, ^0.4.0");
    assert_match_none(r, &["0.0.8", "0.3.0", "0.4.0"]);

    let ref r = req("<= 0.2.0, >= 0.5.0");
    assert_to_string(r, "<=0.2.0, >=0.5.0");
    assert_match_none(r, &["0.0.8", "0.3.0", "0.5.1"]);

    let ref r = req("0.1.0, 0.1.4, 0.1.6");
    assert_to_string(r, "^0.1.0, ^0.1.4, ^0.1.6");
    assert_match_all(r, &["0.1.6", "0.1.9"]);
    assert_match_none(r, &["0.1.0", "0.1.4", "0.2.0"]);

    let err = req_err("> 0.1.0,");
    assert_to_string(
        err,
        "unexpected end of input while parsing major version number",
    );

    let err = req_err("> 0.3.0, ,");
    assert_to_string(
        err,
        "unexpected character ',' while parsing major version number",
    );

    let ref r = req(">=0.5.1-alpha3, <0.6");
    assert_to_string(r, ">=0.5.1-alpha3, <0.6");
    assert_match_all(
        r,
        &[
            "0.5.1-alpha3",
            "0.5.1-alpha4",
            "0.5.1-beta",
            "0.5.1",
            "0.5.5",
        ],
    );
    assert_match_none(
        r,
        &["0.5.1-alpha1", "0.5.2-alpha3", "0.5.5-pre", "0.5.0-pre"],
    );
    assert_match_none(r, &["0.6.0", "0.6.0-pre"]);

    // https://github.com/steveklabnik/semver/issues/56
    let err = req_err("1.2.3 - 2.3.4");
    assert_to_string(err, "expected comma after patch version number, found '-'");

    let err = req_err(">1, >2, >3, >4, >5, >6, >7, >8, >9, >10, >11, >12, >13, >14, >15, >16, >17, >18, >19, >20, >21, >22, >23, >24, >25, >26, >27, >28, >29, >30, >31, >32, >33");
    assert_to_string(err, "excessive number of version comparators");
}

#[test]
pub fn test_whitespace_delimited_comparator_sets() {
    // https://github.com/steveklabnik/semver/issues/55
    let err = req_err("> 0.0.9 <= 2.5.3");
    assert_to_string(err, "expected comma after patch version number, found '<'");
}

#[test]
pub fn test_tilde() {
    let ref r = req("~1");
    assert_match_all(r, &["1.0.0", "1.0.1", "1.1.1"]);
    assert_match_none(r, &["0.9.1", "2.9.0", "0.0.9"]);

    let ref r = req("~1.2");
    assert_match_all(r, &["1.2.0", "1.2.1"]);
    assert_match_none(r, &["1.1.1", "1.3.0", "0.0.9"]);

    let ref r = req("~1.2.2");
    assert_match_all(r, &["1.2.2", "1.2.4"]);
    assert_match_none(r, &["1.2.1", "1.9.0", "1.0.9", "2.0.1", "0.1.3"]);

    let ref r = req("~1.2.3-beta.2");
    assert_match_all(r, &["1.2.3", "1.2.4", "1.2.3-beta.2", "1.2.3-beta.4"]);
    assert_match_none(r, &["1.3.3", "1.1.4", "1.2.3-beta.1", "1.2.4-beta.2"]);
}

#[test]
pub fn test_caret() {
    let ref r = req("^1");
    assert_match_all(r, &["1.1.2", "1.1.0", "1.2.1", "1.0.1"]);
    assert_match_none(r, &["0.9.1", "2.9.0", "0.1.4"]);
    assert_match_none(r, &["1.0.0-beta1", "0.1.0-alpha", "1.0.1-pre"]);

    let ref r = req("^1.1");
    assert_match_all(r, &["1.1.2", "1.1.0", "1.2.1"]);
    assert_match_none(r, &["0.9.1", "2.9.0", "1.0.1", "0.1.4"]);

    let ref r = req("^1.1.2");
    assert_match_all(r, &["1.1.2", "1.1.4", "1.2.1"]);
    assert_match_none(r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1"]);
    assert_match_none(r, &["1.1.2-alpha1", "1.1.3-alpha1", "2.9.0-alpha1"]);

    let ref r = req("^0.1.2");
    assert_match_all(r, &["0.1.2", "0.1.4"]);
    assert_match_none(r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1"]);
    assert_match_none(r, &["0.1.2-beta", "0.1.3-alpha", "0.2.0-pre"]);

    let ref r = req("^0.5.1-alpha3");
    assert_match_all(
        r,
        &[
            "0.5.1-alpha3",
            "0.5.1-alpha4",
            "0.5.1-beta",
            "0.5.1",
            "0.5.5",
        ],
    );
    assert_match_none(
        r,
        &[
            "0.5.1-alpha1",
            "0.5.2-alpha3",
            "0.5.5-pre",
            "0.5.0-pre",
            "0.6.0",
        ],
    );

    let ref r = req("^0.0.2");
    assert_match_all(r, &["0.0.2"]);
    assert_match_none(r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1", "0.1.4"]);

    let ref r = req("^0.0");
    assert_match_all(r, &["0.0.2", "0.0.0"]);
    assert_match_none(r, &["0.9.1", "2.9.0", "1.1.1", "0.1.4"]);

    let ref r = req("^0");
    assert_match_all(r, &["0.9.1", "0.0.2", "0.0.0"]);
    assert_match_none(r, &["2.9.0", "1.1.1"]);

    let ref r = req("^1.4.2-beta.5");
    assert_match_all(
        r,
        &["1.4.2", "1.4.3", "1.4.2-beta.5", "1.4.2-beta.6", "1.4.2-c"],
    );
    assert_match_none(
        r,
        &[
            "0.9.9",
            "2.0.0",
            "1.4.2-alpha",
            "1.4.2-beta.4",
            "1.4.3-beta.5",
        ],
    );
}

#[test]
pub fn test_wildcard() {
    let err = req_err("");
    assert_to_string(
        err,
        "unexpected end of input while parsing major version number",
    );

    let ref r = req("*");
    assert_match_all(r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
    assert_match_none(r, &["1.0.0-pre"]);

    for s in &["x", "X"] {
        assert_eq!(*r, req(s));
    }

    let ref r = req("1.*");
    assert_match_all(r, &["1.2.0", "1.2.1", "1.1.1", "1.3.0"]);
    assert_match_none(r, &["0.0.9", "1.2.0-pre"]);

    for s in &["1.x", "1.X", "1.*.*"] {
        assert_eq!(*r, req(s));
    }

    let ref r = req("1.2.*");
    assert_match_all(r, &["1.2.0", "1.2.2", "1.2.4"]);
    assert_match_none(r, &["1.9.0", "1.0.9", "2.0.1", "0.1.3", "1.2.2-pre"]);

    for s in &["1.2.x", "1.2.X"] {
        assert_eq!(*r, req(s));
    }
}

#[test]
pub fn test_logical_or() {
    // https://github.com/steveklabnik/semver/issues/57
    let err = req_err("=1.2.3 || =2.3.4");
    assert_to_string(err, "expected comma after patch version number, found '|'");

    let err = req_err("1.1 || =1.2.3");
    assert_to_string(err, "expected comma after minor version number, found '|'");

    let err = req_err("6.* || 8.* || >= 10.*");
    assert_to_string(err, "expected comma after minor version number, found '|'");
}

#[test]
pub fn test_any() {
    let ref r = VersionReq::STAR;
    assert_match_all(r, &["0.0.1", "0.1.0", "1.0.0"]);
}

#[test]
pub fn test_pre() {
    let ref r = req("=2.1.1-really.0");
    assert_match_all(r, &["2.1.1-really.0"]);
}

#[test]
pub fn test_parse() {
    let err = req_err("\0");
    assert_to_string(
        err,
        "unexpected character '\\0' while parsing major version number",
    );

    let err = req_err(">= >= 0.0.2");
    assert_to_string(
        err,
        "unexpected character '>' while parsing major version number",
    );

    let err = req_err(">== 0.0.2");
    assert_to_string(
        err,
        "unexpected character '=' while parsing major version number",
    );

    let err = req_err("a.0.0");
    assert_to_string(
        err,
        "unexpected character 'a' while parsing major version number",
    );

    let err = req_err("1.0.0-");
    assert_to_string(err, "empty identifier segment in pre-release identifier");

    let err = req_err(">=");
    assert_to_string(
        err,
        "unexpected end of input while parsing major version number",
    );
}

#[test]
fn test_comparator_parse() {
    let parsed = comparator("1.2.3-alpha");
    assert_to_string(parsed, "^1.2.3-alpha");

    let parsed = comparator("2.X");
    assert_to_string(parsed, "2.*");

    let parsed = comparator("2");
    assert_to_string(parsed, "^2");

    let parsed = comparator("2.x.x");
    assert_to_string(parsed, "2.*");

    let err = comparator_err("1.2.3-01");
    assert_to_string(err, "invalid leading zero in pre-release identifier");

    let err = comparator_err("1.2.3+4.");
    assert_to_string(err, "empty identifier segment in build metadata");

    let err = comparator_err(">");
    assert_to_string(
        err,
        "unexpected end of input while parsing major version number",
    );

    let err = comparator_err("1.");
    assert_to_string(
        err,
        "unexpected end of input while parsing minor version number",
    );

    let err = comparator_err("1.*.");
    assert_to_string(err, "unexpected character after wildcard in version req");

    let err = comparator_err("1.2.3+4ÿ");
    assert_to_string(err, "unexpected character 'ÿ' after build metadata");
}

#[test]
fn test_cargo3202() {
    let ref r = req("0.*.*");
    assert_to_string(r, "0.*");
    assert_match_all(r, &["0.5.0"]);

    let ref r = req("0.0.*");
    assert_to_string(r, "0.0.*");
}

#[test]
fn test_digit_after_wildcard() {
    let err = req_err("*.1");
    assert_to_string(err, "unexpected character after wildcard in version req");

    let err = req_err("1.*.1");
    assert_to_string(err, "unexpected character after wildcard in version req");

    let err = req_err(">=1.*.1");
    assert_to_string(err, "unexpected character after wildcard in version req");
}

#[test]
fn test_eq_hash() {
    fn calculate_hash(value: impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    assert!(req("^1") == req("^1"));
    assert!(calculate_hash(req("^1")) == calculate_hash(req("^1")));
    assert!(req("^1") != req("^2"));
}

#[test]
fn test_leading_digit_in_pre_and_build() {
    for op in &["=", ">", ">=", "<", "<=", "~", "^"] {
        // digit then alpha
        req(&format!("{} 1.2.3-1a", op));
        req(&format!("{} 1.2.3+1a", op));

        // digit then alpha (leading zero)
        req(&format!("{} 1.2.3-01a", op));
        req(&format!("{} 1.2.3+01", op));

        // multiple
        req(&format!("{} 1.2.3-1+1", op));
        req(&format!("{} 1.2.3-1-1+1-1-1", op));
        req(&format!("{} 1.2.3-1a+1a", op));
        req(&format!("{} 1.2.3-1a-1a+1a-1a-1a", op));
    }
}

#[test]
fn test_wildcard_and_another() {
    let err = req_err("*, 0.20.0-any");
    assert_to_string(
        err,
        "wildcard req (*) must be the only comparator in the version req",
    );

    let err = req_err("0.20.0-any, *");
    assert_to_string(
        err,
        "wildcard req (*) must be the only comparator in the version req",
    );

    let err = req_err("0.20.0-any, *, 1.0");
    assert_to_string(
        err,
        "wildcard req (*) must be the only comparator in the version req",
    );
}
