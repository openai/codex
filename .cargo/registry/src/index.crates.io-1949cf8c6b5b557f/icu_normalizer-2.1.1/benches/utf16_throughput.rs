// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};

use criterion::{black_box, Criterion, Throughput};

use smallvec::SmallVec;

//use detone::IterDecomposeVietnamese;

// 2048 times size of u16 fits on one 4KB memory page, which maximizes
// the run to take average over without introducing cross-page effects.
const INPUT_SIZE: usize = 2048;

fn generate_bmp_input_nfc(s: &str) -> Vec<u16> {
    ComposingNormalizerBorrowed::new_nfc()
        .normalize_iter(s.chars().cycle())
        .take(INPUT_SIZE)
        .map(|c| {
            if c <= '\u{FFFF}' {
                c as u16
            } else {
                unreachable!("Data should stay on the BMP!")
            }
        })
        .collect()
}

fn generate_bmp_input_nfd(s: &str) -> Vec<u16> {
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize_iter(s.chars().cycle())
        .take(INPUT_SIZE)
        .map(|c| {
            if c <= '\u{FFFF}' {
                c as u16
            } else {
                unreachable!("Data should stay on the BMP!")
            }
        })
        .collect()
}

/// Removes headers and replaces line feed with space.
/// Do not use for languages that don't use spaces!
fn prepare_file_contents(content: &str) -> String {
    content
        .lines()
        .filter(|&s| !s.starts_with('#'))
        .map(|s| s.to_owned())
        .collect::<Vec<String>>()
        .join(" ")
}

fn slice_as_slice(s: &[u16]) -> &[u16] {
    black_box(s)
}

fn bench_lang(name: &str, data: &str, c: &mut Criterion) {
    let input_nfc = generate_bmp_input_nfc(data);
    let input_nfd = generate_bmp_input_nfd(data);
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let nfd = DecomposingNormalizerBorrowed::new_nfd();

    // Appending to this output is infallible (does not return `Err`) and
    // this is sized to be large enough not to actually take the the heap
    // allocation path.
    let mut output: SmallVec<[u16; INPUT_SIZE * 2]> = SmallVec::new();
    {
        let mut group_name = "utf16_throughput_nfc_".to_string();
        group_name.push_str(name);

        let mut group = c.benchmark_group(&group_name);
        group.throughput(Throughput::Elements(input_nfc.len() as u64));

        group.bench_function("read", |b| {
            b.iter(|| {
                let _ = black_box(
                    nfc.split_normalized_utf16(slice_as_slice(&input_nfc))
                        .0
                        .len(),
                );
            })
        });

        group.bench_function("writing_to_nfc", |b| {
            b.iter(|| {
                output.clear(); // Should be trivial and OK to do from within here.
                let _ = black_box(
                    nfc.normalize_utf16_to(slice_as_slice(&input_nfc), black_box(&mut output)),
                );
            })
        });
        group.bench_function("writing_to_nfd", |b| {
            b.iter(|| {
                output.clear(); // Should be trivial and OK to do from within here.
                let _ = black_box(
                    nfd.normalize_utf16_to(slice_as_slice(&input_nfc), black_box(&mut output)),
                );
            })
        });
        group.finish();
    }
    {
        let mut group_name = "utf16_throughput_nfd_".to_string();
        group_name.push_str(name);

        let mut group = c.benchmark_group(&group_name);
        group.throughput(Throughput::Elements(input_nfd.len() as u64));

        group.bench_function("read", |b| {
            b.iter(|| {
                let _ = black_box(
                    nfd.split_normalized_utf16(slice_as_slice(&input_nfd))
                        .0
                        .len(),
                );
            })
        });

        group.bench_function("writing_to_nfd", |b| {
            b.iter(|| {
                output.clear(); // Should be trivial and OK to do from within here.
                let _ = black_box(
                    nfd.normalize_utf16_to(slice_as_slice(&input_nfd), black_box(&mut output)),
                );
            })
        });
        group.bench_function("writing_to_nfc", |b| {
            b.iter(|| {
                output.clear(); // Should be trivial and OK to do from within here.
                let _ = black_box(
                    nfc.normalize_utf16_to(slice_as_slice(&input_nfd), black_box(&mut output)),
                );
            })
        });
        group.finish();
    }
}

static EL: &str = include_str!("./data/TestRandomWordsUDHR_el.txt");
static EN: &str = "The ICU4X normalizer is an implementation of Unicode Normalization Forms. ";
static FR: &str = include_str!("./data/TestRandomWordsUDHR_fr.txt");
static VI: &str = include_str!("./data/wotw.txt");
static ZH: &str = "單父人呂公善沛令，辟仇，從之客，因家焉。沛中豪傑吏聞令有重客，皆往賀。";
// zh text from https://www.gutenberg.org/cache/epub/23841/pg23841.txt
// metadata at https://www.gutenberg.org/ebooks/23841
// If you replace this text, be sure not to include ASCII spaces and be sure
// to include punctuation using code points actually used for punctuation in
// Chinese.

// TODO: Add:
// * Japanese with realistic proportion of kana voicing marks
// * Korean, since Hangul is special-cased in the normalizer
// * Kannada or some other non-Korean BMP language that uses
//   backward-combining starters (with realistic proportion of such
//   characters).
// * Chakma or some other living non-BMP language.
// * Vietnamese in the orthographic form (i.e. as produced by
//   the official non-IME keyboard layout that's less common
//   than the NFC-producing IME.)

pub fn criterion_benchmark(c: &mut Criterion) {
    bench_lang("el", prepare_file_contents(EL).as_str(), c);
    bench_lang("en", EN, c);
    bench_lang("fr", prepare_file_contents(FR).as_str(), c);
    bench_lang("vi", prepare_file_contents(VI).as_str(), c);
    bench_lang("zh", ZH, c);
}
