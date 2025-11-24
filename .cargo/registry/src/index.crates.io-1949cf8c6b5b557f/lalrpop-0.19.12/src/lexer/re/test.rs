use super::*;

#[test]
fn parse_unclosed_group() {
    parse_regex(r"(123").unwrap_err();
}

#[test]
fn alt_oom() {
    parse_regex(r"(%%|[^%])+").unwrap();
}
