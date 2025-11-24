mod parser;
mod parser_ast;
mod parser_lexer;

use winnow::prelude::*;

#[allow(clippy::eq_op, clippy::erasing_op)]
fn arithmetic(c: &mut criterion::Criterion) {
    let data = "  2*2 / ( 5 - 1) + 3 / 4 * (2 - 7 + 567 *12 /2) + 3*(1+2*( 45 /2))";
    let expected = 2 * 2 / (5 - 1) + 3 * (1 + 2 * (45 / 2));

    assert_eq!(parser::expr.parse(data), Ok(expected));
    assert_eq!(
        parser_ast::expr.parse(data).map(|ast| ast.eval()),
        Ok(expected)
    );
    assert_eq!(
        parser_lexer::expr2.parse(data).map(|ast| ast.eval()),
        Ok(expected)
    );
    c.bench_function("direct", |b| {
        b.iter(|| parser::expr.parse(data).unwrap());
    });
    c.bench_function("ast", |b| {
        b.iter(|| parser_ast::expr.parse(data).unwrap().eval());
    });
    c.bench_function("lexer", |b| {
        b.iter(|| parser_lexer::expr2.parse_peek(data).unwrap());
    });
}

criterion::criterion_group!(benches, arithmetic);
criterion::criterion_main!(benches);
