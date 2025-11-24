// Copyright 2016 The Fancy Regex Authors.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

#[macro_use]
extern crate criterion;

use criterion::Criterion;
use std::time::Duration;

use fancy_regex::internal::{analyze, compile, run_default};
use fancy_regex::Expr;
use regex::Regex;

fn parse_lifetime_re(c: &mut Criterion) {
    c.bench_function("parse_lifetime_re", |b| {
        b.iter(|| Expr::parse_tree("\\'[a-zA-Z_][a-zA-Z0-9_]*(?!\\')\\b").unwrap())
    });
}

fn parse_literal_re(c: &mut Criterion) {
    c.bench_function("parse_literal_re", |b| {
        b.iter(|| Expr::parse_tree("^\\\\([!-/:-@\\[-`\\{-~aftnrv]|[0-7]{1,3}|x[0-9a-fA-F]{2}|x\\{[0-9a-fA-F]{1,6}\\})").unwrap())
    });
}

fn parse_literal_re_regex(c: &mut Criterion) {
    c.bench_function("parse_literal_re_regex", |b| {
        b.iter(|| Regex::new("^\\\\([!-/:-@\\[-`\\{-~aftnrv]|[0-7]{1,3}|x[0-9a-fA-F]{2}|x\\{[0-9a-fA-F]{1,6}\\})").unwrap())
    });
}

fn parse_misc(c: &mut Criterion) {
    c.bench_function("parse_misc", |b| {
        b.iter(|| Expr::parse_tree("^\\p{L}|\\p{N}|\\s|.|\\d").unwrap())
    });
}

fn analyze_literal_re(c: &mut Criterion) {
    let re = "^\\\\([!-/:-@\\[-`\\{-~aftnrv]|[0-7]{1,3}|x[0-9a-fA-F]{2}|x\\{[0-9a-fA-F]{1,6}\\})";
    let tree = Expr::parse_tree(re).unwrap();
    c.bench_function("analyze_literal_re", |b| b.iter(|| analyze(&tree).unwrap()));
}

fn run_backtrack(c: &mut Criterion) {
    let tree = Expr::parse_tree("^.*?(([ab]+)\\1b)").unwrap();
    let a = analyze(&tree).unwrap();
    let p = compile(&a).unwrap();
    c.bench_function("run_backtrack", |b| {
        b.iter(|| {
            let result = run_default(&p, "babab", 0).unwrap();
            assert_eq!(result, Some(vec![0, 5, 0, 2]));
            return result;
        })
    });
}

// The following regex is a pathological case for backtracking
// implementations, see README.md:
fn run_tricky(c: &mut Criterion) {
    let tree = Expr::parse_tree("(a|b|ab)*bc").unwrap();
    let a = analyze(&tree).unwrap();
    let p = compile(&a).unwrap();
    let mut s = String::new();
    for _ in 0..28 {
        s.push_str("ab");
    }
    s.push_str("ac");
    c.bench_function("run_tricky", |b| b.iter(|| run_default(&p, &s, 0).unwrap()));
}

fn run_backtrack_limit(c: &mut Criterion) {
    let tree = Expr::parse_tree("(?i)(a|b|ab)*(?=c)").unwrap();
    let a = analyze(&tree).unwrap();
    let p = compile(&a).unwrap();
    let s = "abababababababababababababababababababababababababababab";
    c.bench_function("run_backtrack_limit", |b| {
        b.iter(|| run_default(&p, &s, 0).unwrap_err())
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(10));
    targets = parse_lifetime_re,
    parse_literal_re,
    parse_literal_re_regex,
    parse_misc,
    analyze_literal_re,
    run_backtrack,
    run_tricky,
);
criterion_group!(
    name = slow_benches;
    config = Criterion::default().sample_size(10);
    targets = run_backtrack_limit,
);

criterion_main!(benches, slow_benches);
