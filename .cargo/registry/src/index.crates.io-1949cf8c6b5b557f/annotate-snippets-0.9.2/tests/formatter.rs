use annotate_snippets::display_list::*;
use annotate_snippets::snippet::{self, Snippet};

#[test]
fn test_source_empty() {
    let dl = DisplayList::from(vec![DisplayLine::Source {
        lineno: None,
        inline_marks: vec![],
        line: DisplaySourceLine::Empty,
    }]);

    assert_eq!(dl.to_string(), " |");
}

#[test]
fn test_source_content() {
    let dl = DisplayList::from(vec![
        DisplayLine::Source {
            lineno: Some(56),
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: "This is an example",
                range: (0, 19),
            },
        },
        DisplayLine::Source {
            lineno: Some(57),
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: "of content lines",
                range: (0, 19),
            },
        },
    ]);

    assert_eq!(
        dl.to_string(),
        "56 | This is an example\n57 | of content lines"
    );
}

#[test]
fn test_source_annotation_standalone_singleline() {
    let dl = DisplayList::from(vec![DisplayLine::Source {
        lineno: None,
        inline_marks: vec![],
        line: DisplaySourceLine::Annotation {
            range: (0, 5),
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::None,
                id: None,
                label: vec![DisplayTextFragment {
                    content: "Example string",
                    style: DisplayTextStyle::Regular,
                }],
            },
            annotation_type: DisplayAnnotationType::Error,
            annotation_part: DisplayAnnotationPart::Standalone,
        },
    }]);

    assert_eq!(dl.to_string(), " | ^^^^^ Example string");
}

#[test]
fn test_source_annotation_standalone_multiline() {
    let dl = DisplayList::from(vec![
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Help,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "Example string",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Warning,
                annotation_part: DisplayAnnotationPart::Standalone,
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Help,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "Second line",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Warning,
                annotation_part: DisplayAnnotationPart::LabelContinuation,
            },
        },
    ]);

    assert_eq!(
        dl.to_string(),
        " | ----- help: Example string\n |             Second line"
    );
}

#[test]
fn test_source_annotation_standalone_multi_annotation() {
    let dl = DisplayList::from(vec![
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Info,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "Example string",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Note,
                annotation_part: DisplayAnnotationPart::Standalone,
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Info,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "Second line",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Note,
                annotation_part: DisplayAnnotationPart::LabelContinuation,
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Warning,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "This is a note",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Note,
                annotation_part: DisplayAnnotationPart::Consequitive,
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Warning,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "Second line of the warning",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Note,
                annotation_part: DisplayAnnotationPart::LabelContinuation,
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Info,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "This is an info",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Info,
                annotation_part: DisplayAnnotationPart::Standalone,
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 5),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::Help,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "This is help",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::Help,
                annotation_part: DisplayAnnotationPart::Standalone,
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Annotation {
                range: (0, 0),
                annotation: Annotation {
                    annotation_type: DisplayAnnotationType::None,
                    id: None,
                    label: vec![DisplayTextFragment {
                        content: "This is an annotation of type none",
                        style: DisplayTextStyle::Regular,
                    }],
                },
                annotation_type: DisplayAnnotationType::None,
                annotation_part: DisplayAnnotationPart::Standalone,
            },
        },
    ]);

    assert_eq!(dl.to_string(), " | ----- info: Example string\n |             Second line\n |       warning: This is a note\n |                Second line of the warning\n | ----- info: This is an info\n | ----- help: This is help\n |  This is an annotation of type none");
}

#[test]
fn test_fold_line() {
    let dl = DisplayList::from(vec![
        DisplayLine::Source {
            lineno: Some(5),
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: "This is line 5",
                range: (0, 19),
            },
        },
        DisplayLine::Fold {
            inline_marks: vec![],
        },
        DisplayLine::Source {
            lineno: Some(10021),
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: "... and now we're at line 10021",
                range: (0, 19),
            },
        },
    ]);

    assert_eq!(
        dl.to_string(),
        "    5 | This is line 5\n...\n10021 | ... and now we're at line 10021"
    );
}

#[test]
fn test_raw_origin_initial_nopos() {
    let dl = DisplayList::from(vec![DisplayLine::Raw(DisplayRawLine::Origin {
        path: "src/test.rs",
        pos: None,
        header_type: DisplayHeaderType::Initial,
    })]);

    assert_eq!(dl.to_string(), "--> src/test.rs");
}

#[test]
fn test_raw_origin_initial_pos() {
    let dl = DisplayList::from(vec![DisplayLine::Raw(DisplayRawLine::Origin {
        path: "src/test.rs",
        pos: Some((23, 15)),
        header_type: DisplayHeaderType::Initial,
    })]);

    assert_eq!(dl.to_string(), "--> src/test.rs:23:15");
}

#[test]
fn test_raw_origin_continuation() {
    let dl = DisplayList::from(vec![DisplayLine::Raw(DisplayRawLine::Origin {
        path: "src/test.rs",
        pos: Some((23, 15)),
        header_type: DisplayHeaderType::Continuation,
    })]);

    assert_eq!(dl.to_string(), "::: src/test.rs:23:15");
}

#[test]
fn test_raw_annotation_unaligned() {
    let dl = DisplayList::from(vec![DisplayLine::Raw(DisplayRawLine::Annotation {
        annotation: Annotation {
            annotation_type: DisplayAnnotationType::Error,
            id: Some("E0001"),
            label: vec![DisplayTextFragment {
                content: "This is an error",
                style: DisplayTextStyle::Regular,
            }],
        },
        source_aligned: false,
        continuation: false,
    })]);

    assert_eq!(dl.to_string(), "error[E0001]: This is an error");
}

#[test]
fn test_raw_annotation_unaligned_multiline() {
    let dl = DisplayList::from(vec![
        DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::Warning,
                id: Some("E0001"),
                label: vec![DisplayTextFragment {
                    content: "This is an error",
                    style: DisplayTextStyle::Regular,
                }],
            },
            source_aligned: false,
            continuation: false,
        }),
        DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::Warning,
                id: Some("E0001"),
                label: vec![DisplayTextFragment {
                    content: "Second line of the error",
                    style: DisplayTextStyle::Regular,
                }],
            },
            source_aligned: false,
            continuation: true,
        }),
    ]);

    assert_eq!(
        dl.to_string(),
        "warning[E0001]: This is an error\n                Second line of the error"
    );
}

#[test]
fn test_raw_annotation_aligned() {
    let dl = DisplayList::from(vec![DisplayLine::Raw(DisplayRawLine::Annotation {
        annotation: Annotation {
            annotation_type: DisplayAnnotationType::Error,
            id: Some("E0001"),
            label: vec![DisplayTextFragment {
                content: "This is an error",
                style: DisplayTextStyle::Regular,
            }],
        },
        source_aligned: true,
        continuation: false,
    })]);

    assert_eq!(dl.to_string(), " = error[E0001]: This is an error");
}

#[test]
fn test_raw_annotation_aligned_multiline() {
    let dl = DisplayList::from(vec![
        DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::Warning,
                id: Some("E0001"),
                label: vec![DisplayTextFragment {
                    content: "This is an error",
                    style: DisplayTextStyle::Regular,
                }],
            },
            source_aligned: true,
            continuation: false,
        }),
        DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::Warning,
                id: Some("E0001"),
                label: vec![DisplayTextFragment {
                    content: "Second line of the error",
                    style: DisplayTextStyle::Regular,
                }],
            },
            source_aligned: true,
            continuation: true,
        }),
    ]);

    assert_eq!(
        dl.to_string(),
        " = warning[E0001]: This is an error\n                   Second line of the error"
    );
}

#[test]
fn test_different_annotation_types() {
    let dl = DisplayList::from(vec![
        DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::Note,
                id: None,
                label: vec![DisplayTextFragment {
                    content: "This is a note",
                    style: DisplayTextStyle::Regular,
                }],
            },
            source_aligned: false,
            continuation: false,
        }),
        DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::None,
                id: None,
                label: vec![DisplayTextFragment {
                    content: "This is just a string",
                    style: DisplayTextStyle::Regular,
                }],
            },
            source_aligned: false,
            continuation: false,
        }),
        DisplayLine::Raw(DisplayRawLine::Annotation {
            annotation: Annotation {
                annotation_type: DisplayAnnotationType::None,
                id: None,
                label: vec![DisplayTextFragment {
                    content: "Second line of none type annotation",
                    style: DisplayTextStyle::Regular,
                }],
            },
            source_aligned: false,
            continuation: true,
        }),
    ]);

    assert_eq!(
        dl.to_string(),
        "note: This is a note\nThis is just a string\n  Second line of none type annotation",
    );
}

#[test]
fn test_inline_marks_empty_line() {
    let dl = DisplayList::from(vec![DisplayLine::Source {
        lineno: None,
        inline_marks: vec![DisplayMark {
            mark_type: DisplayMarkType::AnnotationThrough,
            annotation_type: DisplayAnnotationType::Error,
        }],
        line: DisplaySourceLine::Empty,
    }]);

    assert_eq!(dl.to_string(), " | |",);
}

#[test]
fn test_anon_lines() {
    let mut dl = DisplayList::from(vec![
        DisplayLine::Source {
            lineno: Some(56),
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: "This is an example",
                range: (0, 19),
            },
        },
        DisplayLine::Source {
            lineno: Some(57),
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: "of content lines",
                range: (0, 19),
            },
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Empty,
        },
        DisplayLine::Source {
            lineno: None,
            inline_marks: vec![],
            line: DisplaySourceLine::Content {
                text: "abc",
                range: (0, 19),
            },
        },
    ]);

    dl.anonymized_line_numbers = true;
    assert_eq!(
        dl.to_string(),
        "LL | This is an example\nLL | of content lines\n   |\n   | abc"
    );
}

#[test]
fn test_raw_origin_initial_pos_anon_lines() {
    let mut dl = DisplayList::from(vec![DisplayLine::Raw(DisplayRawLine::Origin {
        path: "src/test.rs",
        pos: Some((23, 15)),
        header_type: DisplayHeaderType::Initial,
    })]);

    // Using anonymized_line_numbers should not affect the inital position
    dl.anonymized_line_numbers = true;
    assert_eq!(dl.to_string(), "--> src/test.rs:23:15");
}

#[test]
fn test_i_29() {
    let snippets = Snippet {
        title: Some(snippet::Annotation {
            id: None,
            label: Some("oops"),
            annotation_type: snippet::AnnotationType::Error,
        }),
        footer: vec![],
        slices: vec![snippet::Slice {
            source: "First line\r\nSecond oops line",
            line_start: 1,
            origin: Some("<current file>"),
            annotations: vec![snippet::SourceAnnotation {
                range: (19, 23),
                label: "oops",
                annotation_type: snippet::AnnotationType::Error,
            }],
            fold: true,
        }],
        opt: Default::default(),
    };
    let expected = r#"error: oops
 --> <current file>:2:8
  |
1 | First line
2 | Second oops line
  |        ^^^^ oops
  |"#;

    assert_eq!(DisplayList::from(snippets).to_string(), expected);
}

#[test]
fn test_point_to_double_width_characters() {
    let snippets = Snippet {
        slices: vec![snippet::Slice {
            source: "こんにちは、世界",
            line_start: 1,
            origin: Some("<current file>"),
            annotations: vec![snippet::SourceAnnotation {
                range: (6, 8),
                label: "world",
                annotation_type: snippet::AnnotationType::Error,
            }],
            fold: false,
        }],
        title: None,
        footer: vec![],
        opt: Default::default(),
    };

    let expected = r#" --> <current file>:1:7
  |
1 | こんにちは、世界
  |             ^^^^ world
  |"#;

    assert_eq!(DisplayList::from(snippets).to_string(), expected);
}

#[test]
fn test_point_to_double_width_characters_across_lines() {
    let snippets = Snippet {
        slices: vec![snippet::Slice {
            source: "おはよう\nございます",
            line_start: 1,
            origin: Some("<current file>"),
            annotations: vec![snippet::SourceAnnotation {
                range: (2, 8),
                label: "Good morning",
                annotation_type: snippet::AnnotationType::Error,
            }],
            fold: false,
        }],
        title: None,
        footer: vec![],
        opt: Default::default(),
    };

    let expected = r#" --> <current file>:1:3
  |
1 |   おはよう
  |  _____^
2 | | ございます
  | |______^ Good morning
  |"#;

    assert_eq!(DisplayList::from(snippets).to_string(), expected);
}

#[test]
fn test_point_to_double_width_characters_multiple() {
    let snippets = Snippet {
        slices: vec![snippet::Slice {
            source: "お寿司\n食べたい🍣",
            line_start: 1,
            origin: Some("<current file>"),
            annotations: vec![
                snippet::SourceAnnotation {
                    range: (0, 3),
                    label: "Sushi1",
                    annotation_type: snippet::AnnotationType::Error,
                },
                snippet::SourceAnnotation {
                    range: (6, 8),
                    label: "Sushi2",
                    annotation_type: snippet::AnnotationType::Note,
                },
            ],
            fold: false,
        }],
        title: None,
        footer: vec![],
        opt: Default::default(),
    };

    let expected = r#" --> <current file>:1:1
  |
1 | お寿司
  | ^^^^^^ Sushi1
2 | 食べたい🍣
  |     ---- note: Sushi2
  |"#;

    assert_eq!(DisplayList::from(snippets).to_string(), expected);
}

#[test]
fn test_point_to_double_width_characters_mixed() {
    let snippets = Snippet {
        slices: vec![snippet::Slice {
            source: "こんにちは、新しいWorld！",
            line_start: 1,
            origin: Some("<current file>"),
            annotations: vec![snippet::SourceAnnotation {
                range: (6, 14),
                label: "New world",
                annotation_type: snippet::AnnotationType::Error,
            }],
            fold: false,
        }],
        title: None,
        footer: vec![],
        opt: Default::default(),
    };

    let expected = r#" --> <current file>:1:7
  |
1 | こんにちは、新しいWorld！
  |             ^^^^^^^^^^^ New world
  |"#;

    assert_eq!(DisplayList::from(snippets).to_string(), expected);
}
