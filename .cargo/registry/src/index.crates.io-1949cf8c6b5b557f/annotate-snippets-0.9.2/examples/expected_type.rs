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
                    range: (193, 195),
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
