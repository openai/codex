# annotate-snippets

`annotate-snippets` is a Rust library for annotation of programming code slices.

[![crates.io](https://img.shields.io/crates/v/annotate-snippets.svg)](https://crates.io/crates/annotate-snippets)
![build status](https://github.com/rust-lang/annotate-snippets-rs/actions/workflows/ci.yml/badge.svg)
[![Coverage Status](https://coveralls.io/repos/github/rust-lang/annotate-snippets-rs/badge.svg?branch=master)](https://coveralls.io/github/rust-lang/annotate-snippets-rs?branch=master)

The library helps visualize meta information annotating source code slices.
It takes a data structure called `Snippet` on the input and produces a `String`
which may look like this:

```text
error[E0308]: mismatched types
  --> src/format.rs:52:1
   |
51 |   ) -> Option<String> {
   |        -------------- expected `Option<String>` because of return type
52 | /     for ann in annotations {
53 | |         match (ann.range.0, ann.range.1) {
54 | |             (None, None) => continue,
55 | |             (Some(start), Some(end)) if start > end_index => continue,
...  |
71 | |         }
72 | |     }
   | |_____^ expected enum `std::option::Option`, found ()
```

[Documentation][]

[Documentation]: https://docs.rs/annotate-snippets/

Usage
-----

```rust
use annotate_snippets::{
    display_list::{DisplayList, FormatOptions},
    snippet::{Annotation, AnnotationType, Slice, Snippet, SourceAnnotation},
};

fn main() {
    let snippet = Snippet {
        title: Some(Annotation {
            label: Some("expected type, found `22`"),
            id: None,
            annotation_type: AnnotationType::Error,
        }),
        footer: vec![],
        slices: vec![Slice {
            source: r#"                annotations: vec![SourceAnnotation {
                label: "expected struct `annotate_snippets::snippet::Slice`, found reference"
                    ,
                range: <22, 25>,"#,
            line_start: 26,
            origin: Some("examples/footer.rs"),
            fold: true,
            annotations: vec![
                SourceAnnotation {
                    label: "",
                    annotation_type: AnnotationType::Error,
                    range: (187, 189),
                },
                SourceAnnotation {
                    label: "while parsing this struct",
                    annotation_type: AnnotationType::Info,
                    range: (34, 50),
                },
            ],
        }],
        opt: FormatOptions {
            color: true,
            ..Default::default()
        },
    };

    let dl = DisplayList::from(snippet);
    println!("{}", dl);
}
```

Local Development
-----------------

    cargo build
    cargo test

When submitting a PR please use  [`cargo fmt`][] (nightly).

[`cargo fmt`]: https://github.com/rust-lang/rustfmt
