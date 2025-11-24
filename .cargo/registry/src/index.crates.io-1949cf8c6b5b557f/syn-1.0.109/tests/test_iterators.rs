use syn::punctuated::{Pair, Punctuated};
use syn::Token;

#[macro_use]
mod macros;

macro_rules! check_exact_size_iterator {
    ($iter:expr) => {{
        let iter = $iter;
        let size_hint = iter.size_hint();
        let len = iter.len();
        let count = iter.count();
        assert_eq!(len, count);
        assert_eq!(size_hint, (count, Some(count)));
    }};
}

#[test]
fn pairs() {
    let mut p: Punctuated<_, Token![,]> = punctuated!(2, 3, 4);

    check_exact_size_iterator!(p.pairs());
    check_exact_size_iterator!(p.pairs_mut());
    check_exact_size_iterator!(p.into_pairs());

    let mut p: Punctuated<_, Token![,]> = punctuated!(2, 3, 4);

    assert_eq!(p.pairs().next_back().map(Pair::into_value), Some(&4));
    assert_eq!(
        p.pairs_mut().next_back().map(Pair::into_value),
        Some(&mut 4)
    );
    assert_eq!(p.into_pairs().next_back().map(Pair::into_value), Some(4));
}

#[test]
fn iter() {
    let mut p: Punctuated<_, Token![,]> = punctuated!(2, 3, 4);

    check_exact_size_iterator!(p.iter());
    check_exact_size_iterator!(p.iter_mut());
    check_exact_size_iterator!(p.into_iter());

    let mut p: Punctuated<_, Token![,]> = punctuated!(2, 3, 4);

    assert_eq!(p.iter().next_back(), Some(&4));
    assert_eq!(p.iter_mut().next_back(), Some(&mut 4));
    assert_eq!(p.into_iter().next_back(), Some(4));
}

#[test]
fn may_dangle() {
    let p: Punctuated<_, Token![,]> = punctuated!(2, 3, 4);
    for element in &p {
        if *element == 2 {
            drop(p);
            break;
        }
    }

    let mut p: Punctuated<_, Token![,]> = punctuated!(2, 3, 4);
    for element in &mut p {
        if *element == 2 {
            drop(p);
            break;
        }
    }
}
