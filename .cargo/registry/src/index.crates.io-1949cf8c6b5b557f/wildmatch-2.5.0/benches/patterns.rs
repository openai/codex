use criterion::{black_box, criterion_group, criterion_main, Criterion};
use glob::Pattern;
use regex::Regex;
use regex_lite::Regex as RegexLite;
use wildmatch::WildMatch;

const TEXT: &str = "Lorem ipsum dolor sit amet, \
consetetur sadipscing elitr, sed diam nonumy eirmod tempor \
invidunt ut labore et dolore magna aliquyam erat, sed diam \
voluptua. At vero eos et accusam et justo duo dolores et ea \
rebum. Stet clita kasd gubergren, no sea takimata sanctus est \
Lorem ipsum dolor sit amet.";

const FULL_TEXT_PATTERN: &str = TEXT;
const FULL_TEXT_REGEX: &str = "^Lorem ipsum dolor sit amet, \
consetetur sadipscing elitr, sed diam nonumy eirmod tempor \
invidunt ut labore et dolore magna aliquyam erat, sed diam \
voluptua\\. At vero eos et accusam et justo duo dolores et ea \
rebum\\. Stet clita kasd gubergren, no sea takimata sanctus est \
Lorem ipsum dolor sit amet\\.$";

const COMPLEX_PATTERN: &str = "Lorem?ipsum*dolore*ea* ?????ata*.";
const COMPLEX_REGEX: &str = "^Lorem.ipsum.*dolore.*ea.* .....ata.*\\.$";

const MOST_COMPLEX_PATTERN: &str = "?a*b*?**c?d****?e*f*g*?*h?i*?*?**j*******k";
const MOST_COMPLEX_REGEX: &str =
    "^.a.*b.*..*.*c.d.*.*.*.*.e.*f.*g.*..*h.i.*..*..*.*j.*.*.*.*.*.*.*k$";

pub fn compiling(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiling");

    group.bench_function("compile text (wildmatch)", |b| {
        b.iter(|| WildMatch::new(black_box(FULL_TEXT_PATTERN)))
    });
    group.bench_function("compile complex (wildmatch)", |b| {
        b.iter(|| WildMatch::new(black_box(MOST_COMPLEX_PATTERN)))
    });

    group.bench_function("compile text (regex)", |b| {
        b.iter(|| Regex::new(black_box(FULL_TEXT_REGEX)).unwrap())
    });
    group.bench_function("compile complex (regex)", |b| {
        b.iter(|| Regex::new(black_box(MOST_COMPLEX_REGEX)).unwrap())
    });

    group.bench_function("compile text (glob)", |b| {
        b.iter(|| Pattern::new(black_box(FULL_TEXT_PATTERN)))
    });
    group.bench_function("compile complex (glob)", |b| {
        b.iter(|| Pattern::new(black_box(MOST_COMPLEX_PATTERN)))
    });

    group.bench_function("compile text (regex_lite)", |b| {
        b.iter(|| RegexLite::new(black_box(FULL_TEXT_PATTERN)))
    });
    group.bench_function("compile complex (regex_lite)", |b| {
        b.iter(|| RegexLite::new(black_box(MOST_COMPLEX_PATTERN)))
    });
}

pub fn matching(c: &mut Criterion) {
    let pattern1 = WildMatch::new(FULL_TEXT_PATTERN);
    let pattern2 = WildMatch::new(COMPLEX_PATTERN);
    let regex1 = Regex::new(FULL_TEXT_REGEX).unwrap();
    let regex2 = Regex::new(COMPLEX_REGEX).unwrap();
    let glob1 = Pattern::new(FULL_TEXT_PATTERN).unwrap();
    let glob2 = Pattern::new(COMPLEX_PATTERN).unwrap();
    let regex_lite1 = RegexLite::new(FULL_TEXT_PATTERN).unwrap();
    let regex_lite2 = RegexLite::new(COMPLEX_PATTERN).unwrap();

    let mut group = c.benchmark_group("matching");

    group.bench_function("match text (wildmatch)", |b| {
        b.iter(|| pattern1 == black_box(TEXT))
    });
    group.bench_function("match complex (wildmatch)", |b| {
        b.iter(|| pattern2 == black_box(TEXT))
    });

    group.bench_function("match text (regex)", |b| {
        b.iter(|| regex1.is_match(black_box(TEXT)))
    });
    group.bench_function("match complex (regex)", |b| {
        b.iter(|| regex2.is_match(black_box(TEXT)))
    });

    group.bench_function("match text (glob)", |b| {
        b.iter(|| glob1.matches(black_box(TEXT)))
    });
    group.bench_function("match complex (glob)", |b| {
        b.iter(|| glob2.matches(black_box(TEXT)))
    });

    group.bench_function("match text (regex_lite)", |b| {
        b.iter(|| regex_lite1.is_match(black_box(TEXT)))
    });
    group.bench_function("match complex (regex_lite)", |b| {
        b.iter(|| regex_lite2.is_match(black_box(TEXT)))
    });
}

criterion_group!(benches, compiling, matching);
criterion_main!(benches);
