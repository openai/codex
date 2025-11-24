use annotate_snippets::{
    display_list::{DisplayList, FormatOptions},
    snippet::{Annotation, AnnotationType, Slice, Snippet, SourceAnnotation},
};

fn main() {
    let snippet = Snippet {
        title: Some(Annotation {
            label: Some("mismatched types"),
            id: Some("E0308"),
            annotation_type: AnnotationType::Error,
        }),
        footer: vec![Annotation {
            label: Some(
                "expected type: `snippet::Annotation`\n   found type: `__&__snippet::Annotation`",
            ),
            id: None,
            annotation_type: AnnotationType::Note,
        }],
        slices: vec![Slice {
            source: "        slices: vec![\"A\",",
            line_start: 13,
            origin: Some("src/multislice.rs"),
            fold: false,
            annotations: vec![SourceAnnotation {
                label: "expected struct `annotate_snippets::snippet::Slice`, found reference",
                range: (21, 24),
                annotation_type: AnnotationType::Error,
            }],
        }],
        opt: FormatOptions {
            color: true,
            ..Default::default()
        },
    };

    let dl = DisplayList::from(snippet);
    println!("{}", dl);
}
