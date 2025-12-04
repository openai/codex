use tree_sitter::Parser;

use super::helpers::{allocations, fixtures::get_language};

#[test]
fn test_pathological_example_1() {
    let language = "cpp";
    let source = r#"*ss<s"ss<sqXqss<s._<s<sq<(qqX<sqss<s.ss<sqsssq<(qss<qssqXqss<s._<s<sq<(qqX<sqss<s.ss<sqsssq<(qss<sqss<sqss<s._<s<sq>(qqX<sqss<s.ss<sqsssq<(qss<sq&=ss<s<sqss<s._<s<sq<(qqX<sqss<s.ss<sqs"#;

    allocations::record(|| {
        let mut parser = Parser::new();
        parser.set_language(&get_language(language)).unwrap();
        parser.parse(source, None).unwrap();
    });
}
