use annotate_snippets::{
    display_list::{DisplayList, FormatOptions},
    snippet::{Annotation, AnnotationType, Slice, Snippet, SourceAnnotation},
};

fn main() {
    let snippet = Snippet {
        slices: vec![Slice {
            source: r#") -> Option<String> {
    for ann in annotations {
        match (ann.range.0, ann.range.1) {
            (None, None) => continue,
            (Some(start), Some(end)) if start > end_index => continue,
            (Some(start), Some(end)) if start >= start_index => {
                let label = if let Some(ref label) = ann.label {
                    format!(" {}", label)
                } else {
                    String::from("")
                };

                return Some(format!(
                    "{}{}{}",
                    " ".repeat(start - start_index),
                    "^".repeat(end - start),
                    label
                ));
            }
            _ => continue,
        }
    }"#,
            line_start: 51,
            origin: Some("src/format.rs"),
            fold: false,
            annotations: vec![
                SourceAnnotation {
                    label: "expected `Option<String>` because of return type",
                    annotation_type: AnnotationType::Warning,
                    range: (5, 19),
                },
                SourceAnnotation {
                    label: "expected enum `std::option::Option`",
                    annotation_type: AnnotationType::Error,
                    range: (26, 724),
                },
            ],
        }],
        title: Some(Annotation {
            label: Some("mismatched types"),
            id: Some("E0308"),
            annotation_type: AnnotationType::Error,
        }),
        footer: vec![],
        opt: FormatOptions {
            color: true,
            ..Default::default()
        },
    };

    let dl = DisplayList::from(snippet);
    println!("{}", dl);
}
