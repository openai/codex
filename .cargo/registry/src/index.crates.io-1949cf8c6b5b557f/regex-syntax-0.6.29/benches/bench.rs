#![feature(test)]

extern crate test;

use regex_syntax::Parser;
use test::Bencher;

#[bench]
fn parse_simple1(b: &mut Bencher) {
    b.iter(|| {
        let re = r"^bc(d|e)*$";
        Parser::new().parse(re).unwrap()
    });
}

#[bench]
fn parse_simple2(b: &mut Bencher) {
    b.iter(|| {
        let re = r"'[a-zA-Z_][a-zA-Z0-9_]*(')\b";
        Parser::new().parse(re).unwrap()
    });
}

#[bench]
fn parse_small1(b: &mut Bencher) {
    b.iter(|| {
        let re = r"\p{L}|\p{N}|\s|.|\d";
        Parser::new().parse(re).unwrap()
    });
}

#[bench]
fn parse_medium1(b: &mut Bencher) {
    b.iter(|| {
        let re = r"\pL\p{Greek}\p{Hiragana}\p{Alphabetic}\p{Hebrew}\p{Arabic}";
        Parser::new().parse(re).unwrap()
    });
}

#[bench]
fn parse_medium2(b: &mut Bencher) {
    b.iter(|| {
        let re = r"\s\S\w\W\d\D";
        Parser::new().parse(re).unwrap()
    });
}

#[bench]
fn parse_medium3(b: &mut Bencher) {
    b.iter(|| {
        let re =
            r"\p{age:3.2}\p{hira}\p{scx:hira}\p{alphabetic}\p{sc:Greek}\pL";
        Parser::new().parse(re).unwrap()
    });
}

#[bench]
fn parse_huge(b: &mut Bencher) {
    b.iter(|| {
        let re = r"\p{L}{100}";
        Parser::new().parse(re).unwrap()
    });
}
