use std::fs;
use std::io;
use term::terminfo::TermInfo;
use term::terminfo::TerminfoTerminal;
use term::Terminal;

#[test]
fn test_parse() {
    for f in fs::read_dir("tests/data/").unwrap() {
        let _ = TermInfo::from_path(f.unwrap().path()).unwrap();
    }
}

#[test]
fn test_supports_color() {
    fn supports_color(term: &str) -> bool {
        let terminfo = TermInfo::from_path(format!("tests/data/{}", term)).unwrap();
        let term = TerminfoTerminal::new_with_terminfo(io::stdout(), terminfo);
        term.supports_color()
    }
    assert!(supports_color("linux"));
    assert!(!supports_color("dumb"));
}

#[test]
fn test_fallback() {
    TermInfo::from_name("ansi-cargo-test").expect("failed to use fallback");
    assert!(TermInfo::from_name("really-bad-terminal").is_err());
}
