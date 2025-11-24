#![feature(test)]

extern crate beef;
extern crate test;

use std::borrow::{Cow as StdCow, ToOwned};
use test::{black_box, Bencher};

const NTH_WORD: usize = 4;
static TEXT: &str = "In less than a half-hour, Joe had distributed ninety-two paper cups of tomato juice containing AUM, the drug that promised to turn neophobes into neophiles. He stood in Pioneer Court, just north of the Michigan Avenue Bridge, at a table from which hung a poster reading FREE TOMATO JUICE. Each person who took a cupful was invited to fill out a short questionnaire and leave it in a box on Joe's table. However, Joe explained, the questionnaire was optional, and anyone who wanted to drink the tomato juice and run was welcome to do so.";

#[bench]
fn beef_create(b: &mut Bencher) {
    use beef::Cow;

    let words: Vec<_> = TEXT.split_whitespace().collect();

    b.iter(|| {
        let cow_words: Vec<Cow<str>> = words.iter().copied().map(Cow::borrowed).collect();

        black_box(cow_words)
    });
}

#[bench]
fn beef_create_mixed(b: &mut Bencher) {
    use beef::Cow;

    let words: Vec<_> = TEXT.split_whitespace().collect();

    b.iter(|| {
        let cow_words: Vec<Cow<str>> = words
            .iter()
            .copied()
            .map(|word| {
                if word.len() % NTH_WORD == 0 {
                    Cow::owned(word.to_owned())
                } else {
                    Cow::borrowed(word)
                }
            })
            .collect();

        black_box(cow_words)
    });
}

#[bench]
fn beef_as_ref(b: &mut Bencher) {
    use beef::Cow;

    let cow_words: Vec<_> = TEXT
        .split_whitespace()
        .map(|word| {
            if word.len() % NTH_WORD == 0 {
                Cow::owned(word.to_owned())
            } else {
                Cow::borrowed(word)
            }
        })
        .collect();

    b.iter(|| {
        for word in cow_words.iter() {
            let word: &str = word.as_ref();
            black_box(word);
        }
    });
}

#[bench]
fn lean_beef_create(b: &mut Bencher) {
    use beef::lean::Cow;

    let words: Vec<_> = TEXT.split_whitespace().collect();

    b.iter(|| {
        let cow_words: Vec<Cow<str>> = words.iter().copied().map(Cow::borrowed).collect();

        black_box(cow_words)
    });
}

#[bench]
fn lean_beef_create_mixed(b: &mut Bencher) {
    use beef::lean::Cow;

    let words: Vec<_> = TEXT.split_whitespace().collect();

    b.iter(|| {
        let cow_words: Vec<Cow<str>> = words
            .iter()
            .copied()
            .map(|word| {
                if word.len() % NTH_WORD == 0 {
                    Cow::owned(word.to_owned())
                } else {
                    Cow::borrowed(word)
                }
            })
            .collect();

        black_box(cow_words)
    });
}

#[bench]
fn lean_beef_as_ref(b: &mut Bencher) {
    use beef::lean::Cow;

    let cow_words: Vec<_> = TEXT
        .split_whitespace()
        .map(|word| {
            if word.len() % NTH_WORD == 0 {
                Cow::owned(word.to_owned())
            } else {
                Cow::borrowed(word)
            }
        })
        .collect();

    b.iter(|| {
        for word in cow_words.iter() {
            let word: &str = word.as_ref();
            black_box(word);
        }
    });
}

#[bench]
fn std_create(b: &mut Bencher) {
    let words: Vec<_> = TEXT.split_whitespace().collect();

    b.iter(|| {
        let stdcow_words: Vec<StdCow<str>> = words.iter().copied().map(StdCow::Borrowed).collect();

        black_box(stdcow_words)
    });
}

#[bench]
fn std_create_mixed(b: &mut Bencher) {
    let words: Vec<_> = TEXT.split_whitespace().collect();

    b.iter(|| {
        let stdcow_words: Vec<StdCow<str>> = words
            .iter()
            .copied()
            .map(|word| {
                if word.len() % NTH_WORD == 0 {
                    StdCow::Owned(word.to_owned())
                } else {
                    StdCow::Borrowed(word)
                }
            })
            .collect();

        black_box(stdcow_words)
    });
}

#[bench]
fn std_as_ref(b: &mut Bencher) {
    let stdcow_words: Vec<_> = TEXT
        .split_whitespace()
        .map(|word| {
            if word.len() % NTH_WORD == 0 {
                StdCow::Owned(word.to_owned())
            } else {
                StdCow::Borrowed(word)
            }
        })
        .collect();

    b.iter(|| {
        for word in stdcow_words.iter() {
            let word: &str = word.as_ref();
            black_box(word);
        }
    });
}
