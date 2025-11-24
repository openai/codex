use criterion::{criterion_group, criterion_main, Criterion};
use pulldown_cmark::{html, Parser};
use std::fs::{read_dir, read_to_string};

pub fn markdown_it_samples(c: &mut Criterion) {
    let folder = read_dir("./third_party/markdown-it").unwrap();
    for entry in folder {
        let entry = entry.unwrap();

        if entry.metadata().unwrap().is_file() {
            let filename = &entry.file_name().into_string().unwrap();
            if !filename.ends_with(".md") || filename == "README.md" {
                continue;
            }

            let corpus = read_to_string(entry.path()).unwrap();
            let mut result = String::with_capacity(corpus.len() * 3 / 2);

            c.bench_function(filename, |b| {
                b.iter(|| {
                    html::push_html(&mut result, Parser::new(&corpus));
                })
            });
        }
    }
}

criterion_group!(benches, markdown_it_samples);
criterion_main!(benches);
