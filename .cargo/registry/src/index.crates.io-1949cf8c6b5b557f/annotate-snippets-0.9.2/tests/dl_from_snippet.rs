use annotate_snippets::display_list::DisplayList;
use annotate_snippets::{display_list as dl, formatter::get_term_style, snippet};

#[test]
fn test_format_title() {
    let input = snippet::Snippet {
        title: Some(snippet::Annotation {
            id: Some("E0001"),
            label: Some("This is a title"),
            annotation_type: snippet::AnnotationType::Error,
        }),
        footer: vec![],
        slices: vec![],
        opt: Default::default(),
    };
    let output = dl::DisplayList {
        body: vec![dl::DisplayLine::Raw(dl::DisplayRawLine::Annotation {
            annotation: dl::Annotation {
                annotation_type: dl::DisplayAnnotationType::Error,
                id: Some("E0001"),
                label: vec![dl::DisplayTextFragment {
                    content: "This is a title",
                    style: dl::DisplayTextStyle::Emphasis,
                }],
            },
            source_aligned: false,
            continuation: false,
        })],
        stylesheet: get_term_style(input.opt.color),
        anonymized_line_numbers: input.opt.anonymized_line_numbers,
        margin: None,
    };
    assert_eq!(dl::DisplayList::from(input), output);
}

#[test]
fn test_format_slice() {
    let line_1 = "This is line 1";
    let line_2 = "This is line 2";
    let source = vec![line_1, line_2].join("\n");
    let input = snippet::Snippet {
        title: None,
        footer: vec![],
        slices: vec![snippet::Slice {
            source: &source,
            line_start: 5402,
            origin: None,
            annotations: vec![],
            fold: false,
        }],
        opt: Default::default(),
    };
    let output = dl::DisplayList {
        body: vec![
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
            dl::DisplayLine::Source {
                lineno: Some(5402),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    text: line_1,
                    range: (0, line_1.len()),
                },
            },
            dl::DisplayLine::Source {
                lineno: Some(5403),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    range: (line_1.len() + 1, source.len()),
                    text: line_2,
                },
            },
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
        ],
        stylesheet: get_term_style(input.opt.color),
        anonymized_line_numbers: input.opt.anonymized_line_numbers,
        margin: None,
    };
    assert_eq!(dl::DisplayList::from(input), output);
}

#[test]
fn test_format_slices_continuation() {
    let src_0 = "This is slice 1";
    let src_0_len = src_0.len();
    let src_1 = "This is slice 2";
    let src_1_len = src_1.len();
    let input = snippet::Snippet {
        title: None,
        footer: vec![],
        slices: vec![
            snippet::Slice {
                source: src_0,
                line_start: 5402,
                origin: Some("file1.rs"),
                annotations: vec![],
                fold: false,
            },
            snippet::Slice {
                source: src_1,
                line_start: 2,
                origin: Some("file2.rs"),
                annotations: vec![],
                fold: false,
            },
        ],
        opt: Default::default(),
    };
    let output = dl::DisplayList {
        body: vec![
            dl::DisplayLine::Raw(dl::DisplayRawLine::Origin {
                path: "file1.rs",
                pos: None,
                header_type: dl::DisplayHeaderType::Initial,
            }),
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
            dl::DisplayLine::Source {
                lineno: Some(5402),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    text: src_0,
                    range: (0, src_0_len),
                },
            },
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
            dl::DisplayLine::Raw(dl::DisplayRawLine::Origin {
                path: "file2.rs",
                pos: None,
                header_type: dl::DisplayHeaderType::Continuation,
            }),
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
            dl::DisplayLine::Source {
                lineno: Some(2),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    text: src_1,
                    range: (0, src_1_len),
                },
            },
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
        ],
        stylesheet: get_term_style(input.opt.color),
        anonymized_line_numbers: input.opt.anonymized_line_numbers,
        margin: None,
    };
    assert_eq!(dl::DisplayList::from(input), output);
}

#[test]
fn test_format_slice_annotation_standalone() {
    let line_1 = "This is line 1";
    let line_2 = "This is line 2";
    let source = vec![line_1, line_2].join("\n");
    // In line 2
    let range = (22, 24);
    let input = snippet::Snippet {
        title: None,
        footer: vec![],
        slices: vec![snippet::Slice {
            source: &source,
            line_start: 5402,
            origin: None,
            annotations: vec![snippet::SourceAnnotation {
                range,
                label: "Test annotation",
                annotation_type: snippet::AnnotationType::Info,
            }],
            fold: false,
        }],
        opt: Default::default(),
    };
    let output = dl::DisplayList {
        body: vec![
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
            dl::DisplayLine::Source {
                lineno: Some(5402),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    range: (0, line_1.len()),
                    text: line_1,
                },
            },
            dl::DisplayLine::Source {
                lineno: Some(5403),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    range: (line_1.len() + 1, source.len()),
                    text: line_2,
                },
            },
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Annotation {
                    annotation: dl::Annotation {
                        annotation_type: dl::DisplayAnnotationType::Info,
                        id: None,
                        label: vec![dl::DisplayTextFragment {
                            content: "Test annotation",
                            style: dl::DisplayTextStyle::Regular,
                        }],
                    },
                    range: (range.0 - (line_1.len() + 1), range.1 - (line_1.len() + 1)),
                    annotation_type: dl::DisplayAnnotationType::Info,
                    annotation_part: dl::DisplayAnnotationPart::Standalone,
                },
            },
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
        ],
        stylesheet: get_term_style(input.opt.color),
        anonymized_line_numbers: input.opt.anonymized_line_numbers,
        margin: None,
    };
    assert_eq!(dl::DisplayList::from(input), output);
}

#[test]
fn test_format_label() {
    let input = snippet::Snippet {
        title: None,
        footer: vec![snippet::Annotation {
            id: None,
            label: Some("This __is__ a title"),
            annotation_type: snippet::AnnotationType::Error,
        }],
        slices: vec![],
        opt: Default::default(),
    };
    let output = dl::DisplayList {
        body: vec![dl::DisplayLine::Raw(dl::DisplayRawLine::Annotation {
            annotation: dl::Annotation {
                annotation_type: dl::DisplayAnnotationType::Error,
                id: None,
                label: vec![dl::DisplayTextFragment {
                    content: "This __is__ a title",
                    style: dl::DisplayTextStyle::Regular,
                }],
            },
            source_aligned: true,
            continuation: false,
        })],
        stylesheet: get_term_style(input.opt.color),
        anonymized_line_numbers: input.opt.anonymized_line_numbers,
        margin: None,
    };
    assert_eq!(dl::DisplayList::from(input), output);
}

#[test]
#[should_panic]
fn test_i26() {
    let source = "short";
    let label = "label";
    let input = snippet::Snippet {
        title: None,
        footer: vec![],
        slices: vec![snippet::Slice {
            annotations: vec![snippet::SourceAnnotation {
                range: (0, source.len() + 1),
                label,
                annotation_type: snippet::AnnotationType::Error,
            }],
            source,
            line_start: 0,
            origin: None,
            fold: false,
        }],
        opt: Default::default(),
    };

    let _ = dl::DisplayList::from(input);
}

#[test]
fn test_i_29() {
    let snippets = snippet::Snippet {
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

    let expected = DisplayList {
        body: vec![
            dl::DisplayLine::Raw(dl::DisplayRawLine::Annotation {
                annotation: dl::Annotation {
                    annotation_type: dl::DisplayAnnotationType::Error,
                    id: None,
                    label: vec![dl::DisplayTextFragment {
                        content: "oops",
                        style: dl::DisplayTextStyle::Emphasis,
                    }],
                },
                source_aligned: false,
                continuation: false,
            }),
            dl::DisplayLine::Raw(dl::DisplayRawLine::Origin {
                path: "<current file>",
                pos: Some((2, 8)),
                header_type: dl::DisplayHeaderType::Initial,
            }),
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
            dl::DisplayLine::Source {
                lineno: Some(1),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    text: "First line",
                    range: (0, 10),
                },
            },
            dl::DisplayLine::Source {
                lineno: Some(2),
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Content {
                    text: "Second oops line",
                    range: (12, 28),
                },
            },
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Annotation {
                    annotation: dl::Annotation {
                        annotation_type: dl::DisplayAnnotationType::None,
                        id: None,
                        label: vec![dl::DisplayTextFragment {
                            content: "oops",
                            style: dl::DisplayTextStyle::Regular,
                        }],
                    },
                    range: (7, 11),
                    annotation_type: dl::DisplayAnnotationType::Error,
                    annotation_part: dl::DisplayAnnotationPart::Standalone,
                },
            },
            dl::DisplayLine::Source {
                lineno: None,
                inline_marks: vec![],
                line: dl::DisplaySourceLine::Empty,
            },
        ],
        stylesheet: get_term_style(false),
        anonymized_line_numbers: false,
        margin: None,
    };

    assert_eq!(DisplayList::from(snippets), expected);
}
