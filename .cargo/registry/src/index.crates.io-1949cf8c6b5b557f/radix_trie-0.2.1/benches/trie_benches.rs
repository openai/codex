use criterion::{criterion_group, criterion_main, Criterion};
use radix_trie::Trie;

fn get_text() -> Vec<String> {
    use std::fs::File;
    use std::io::Read;
    const DATA: &[&str] = &["data/1984.txt", "data/sun-rising.txt"];
    let mut contents = String::new();
    File::open(&DATA[1])
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    contents
        .split(|c: char| c.is_whitespace())
        .map(|s| s.to_string())
        .collect()
}

fn make_trie(words: &[String]) -> Trie<&str, usize> {
    let mut trie = Trie::new();
    for w in words {
        trie.insert(&w[..], w.len());
    }
    trie
}

fn trie_insert(b: &mut Criterion) {
    let words = get_text();
    b.bench_function("trie insert", |b| b.iter(|| make_trie(&words)));
}

fn trie_get(b: &mut Criterion) {
    let words = get_text();
    let trie = make_trie(&words);
    b.bench_function("trie get", |b| {
        b.iter(|| {
            words
                .iter()
                .map(|w| trie.get(&&w[..]))
                .collect::<Vec<Option<&usize>>>()
        })
    });
}

fn trie_insert_remove(b: &mut Criterion) {
    let words = get_text();

    b.bench_function("trie remove", |b| {
        b.iter(|| {
            let mut trie = make_trie(&words);
            for w in &words {
                trie.remove(&&w[..]);
            }
        });
    });
}

criterion_group!(benches, trie_insert, trie_get, trie_insert_remove);

criterion_main!(benches);
