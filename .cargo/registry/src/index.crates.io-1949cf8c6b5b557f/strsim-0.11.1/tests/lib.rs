extern crate strsim;

use strsim::{
    damerau_levenshtein, hamming, jaro, jaro_winkler, levenshtein, normalized_damerau_levenshtein,
    normalized_levenshtein, osa_distance,
};

macro_rules! assert_delta {
    ($x:expr, $y:expr) => {
        assert_delta!($x, $y, 1e-5);
    };
    ($x:expr, $y:expr, $d:expr) => {
        if ($x - $y).abs() > $d {
            panic!(
                "assertion failed: actual: `{}`, expected: `{}`: \
                actual not within < {} of expected",
                $x, $y, $d
            );
        }
    };
}

#[test]
fn hamming_works() {
    match hamming("hamming", "hammers") {
        Ok(distance) => assert_eq!(3, distance),
        Err(why) => panic!("{:?}", why),
    }
}

#[test]
fn levenshtein_works() {
    assert_eq!(3, levenshtein("kitten", "sitting"));
}

#[test]
fn normalized_levenshtein_works() {
    assert_delta!(0.57142, normalized_levenshtein("kitten", "sitting"));
}

#[test]
fn osa_distance_works() {
    assert_eq!(3, osa_distance("ac", "cba"));
}

#[test]
fn damerau_levenshtein_works() {
    assert_eq!(2, damerau_levenshtein("ac", "cba"));
}

#[test]
fn normalized_damerau_levenshtein_works() {
    assert_delta!(
        0.27272,
        normalized_damerau_levenshtein("levenshtein", "löwenbräu")
    );
}

#[test]
fn jaro_works() {
    assert_delta!(
        0.392,
        jaro("Friedrich Nietzsche", "Jean-Paul Sartre"),
        0.001
    );
}

#[test]
fn jaro_winkler_works() {
    assert_delta!(0.866, jaro_winkler("cheeseburger", "cheese fries"), 0.001);
}
