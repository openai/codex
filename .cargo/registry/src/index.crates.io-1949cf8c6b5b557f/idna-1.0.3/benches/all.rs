#![allow(deprecated)]

#[macro_use]
extern crate bencher;
extern crate idna;

use bencher::{black_box, Bencher};
use idna::Config;

fn to_unicode_puny_label(bench: &mut Bencher) {
    let encoded = "abc.xn--mgbcm";
    let config = Config::default();
    bench.iter(|| config.to_unicode(black_box(encoded)));
}

fn to_ascii_already_puny_label(bench: &mut Bencher) {
    let encoded = "abc.xn--mgbcm";
    let config = Config::default();
    bench.iter(|| config.to_ascii(black_box(encoded)));
}

fn to_unicode_ascii(bench: &mut Bencher) {
    let encoded = "example.com";
    let config = Config::default();
    bench.iter(|| config.to_unicode(black_box(encoded)));
}

fn to_unicode_merged_label(bench: &mut Bencher) {
    let encoded = "Beispiel.xn--vermgensberater-ctb";
    let config = Config::default();
    bench.iter(|| config.to_unicode(black_box(encoded)));
}

fn to_ascii_puny_label(bench: &mut Bencher) {
    let encoded = "abc.ابج";
    let config = Config::default();
    bench.iter(|| config.to_ascii(black_box(encoded)));
}

fn to_ascii_simple(bench: &mut Bencher) {
    let encoded = "example.com";
    let config = Config::default();
    bench.iter(|| config.to_ascii(black_box(encoded)));
}

fn to_ascii_merged(bench: &mut Bencher) {
    let encoded = "beispiel.vermögensberater";
    let config = Config::default();
    bench.iter(|| config.to_ascii(black_box(encoded)));
}

fn to_ascii_cow_plain(bench: &mut Bencher) {
    let encoded = "example.com".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_hyphen(bench: &mut Bencher) {
    let encoded = "hyphenated-example.com".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_leading_digit(bench: &mut Bencher) {
    let encoded = "1test.example".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_unicode_mixed(bench: &mut Bencher) {
    let encoded = "مثال.example".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_punycode_mixed(bench: &mut Bencher) {
    let encoded = "xn--mgbh0fb.example".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_unicode_ltr(bench: &mut Bencher) {
    let encoded = "නම.උදාහරණ".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_punycode_ltr(bench: &mut Bencher) {
    let encoded = "xn--r0co.xn--ozc8dl2c3bxd".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_unicode_rtl(bench: &mut Bencher) {
    let encoded = "الاسم.مثال".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

fn to_ascii_cow_punycode_rtl(bench: &mut Bencher) {
    let encoded = "xn--mgba0b1dh.xn--mgbh0fb".as_bytes();
    bench.iter(|| idna::domain_to_ascii_cow(black_box(encoded), idna::AsciiDenyList::URL));
}

benchmark_group!(
    benches,
    to_unicode_puny_label,
    to_unicode_ascii,
    to_unicode_merged_label,
    to_ascii_puny_label,
    to_ascii_already_puny_label,
    to_ascii_simple,
    to_ascii_merged,
    to_ascii_cow_plain,
    to_ascii_cow_hyphen,
    to_ascii_cow_leading_digit,
    to_ascii_cow_unicode_mixed,
    to_ascii_cow_punycode_mixed,
    to_ascii_cow_unicode_ltr,
    to_ascii_cow_punycode_ltr,
    to_ascii_cow_unicode_rtl,
    to_ascii_cow_punycode_rtl,
);
benchmark_main!(benches);
