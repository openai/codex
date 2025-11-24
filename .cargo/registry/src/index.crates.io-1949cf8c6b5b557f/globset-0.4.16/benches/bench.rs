/*!
This module benchmarks the glob implementation. For benchmarks on the ripgrep
tool itself, see the benchsuite directory.
*/
#![feature(test)]

extern crate test;

use globset::{Candidate, Glob, GlobMatcher, GlobSet, GlobSetBuilder};

const EXT: &'static str = "some/a/bigger/path/to/the/crazy/needle.txt";
const EXT_PAT: &'static str = "*.txt";

const SHORT: &'static str = "some/needle.txt";
const SHORT_PAT: &'static str = "some/**/needle.txt";

const LONG: &'static str = "some/a/bigger/path/to/the/crazy/needle.txt";
const LONG_PAT: &'static str = "some/**/needle.txt";

fn new_glob(pat: &str) -> glob::Pattern {
    glob::Pattern::new(pat).unwrap()
}

fn new_reglob(pat: &str) -> GlobMatcher {
    Glob::new(pat).unwrap().compile_matcher()
}

fn new_reglob_many(pats: &[&str]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pat in pats {
        builder.add(Glob::new(pat).unwrap());
    }
    builder.build().unwrap()
}

#[bench]
fn ext_glob(b: &mut test::Bencher) {
    let pat = new_glob(EXT_PAT);
    b.iter(|| assert!(pat.matches(EXT)));
}

#[bench]
fn ext_regex(b: &mut test::Bencher) {
    let set = new_reglob(EXT_PAT);
    let cand = Candidate::new(EXT);
    b.iter(|| assert!(set.is_match_candidate(&cand)));
}

#[bench]
fn short_glob(b: &mut test::Bencher) {
    let pat = new_glob(SHORT_PAT);
    b.iter(|| assert!(pat.matches(SHORT)));
}

#[bench]
fn short_regex(b: &mut test::Bencher) {
    let set = new_reglob(SHORT_PAT);
    let cand = Candidate::new(SHORT);
    b.iter(|| assert!(set.is_match_candidate(&cand)));
}

#[bench]
fn long_glob(b: &mut test::Bencher) {
    let pat = new_glob(LONG_PAT);
    b.iter(|| assert!(pat.matches(LONG)));
}

#[bench]
fn long_regex(b: &mut test::Bencher) {
    let set = new_reglob(LONG_PAT);
    let cand = Candidate::new(LONG);
    b.iter(|| assert!(set.is_match_candidate(&cand)));
}

const MANY_SHORT_GLOBS: &'static [&'static str] = &[
    // Taken from a random .gitignore on my system.
    ".*.swp",
    "tags",
    "target",
    "*.lock",
    "tmp",
    "*.csv",
    "*.fst",
    "*-got",
    "*.csv.idx",
    "words",
    "98m*",
    "dict",
    "test",
    "months",
];

const MANY_SHORT_SEARCH: &'static str = "98m-blah.csv.idx";

#[bench]
fn many_short_glob(b: &mut test::Bencher) {
    let pats: Vec<_> = MANY_SHORT_GLOBS.iter().map(|&s| new_glob(s)).collect();
    b.iter(|| {
        let mut count = 0;
        for pat in &pats {
            if pat.matches(MANY_SHORT_SEARCH) {
                count += 1;
            }
        }
        assert_eq!(2, count);
    })
}

#[bench]
fn many_short_regex_set(b: &mut test::Bencher) {
    let set = new_reglob_many(MANY_SHORT_GLOBS);
    b.iter(|| assert_eq!(2, set.matches(MANY_SHORT_SEARCH).iter().count()));
}
