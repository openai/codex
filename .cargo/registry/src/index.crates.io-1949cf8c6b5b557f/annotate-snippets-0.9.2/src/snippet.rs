//! Structures used as an input for the library.
//!
//! Example:
//!
//! ```
//! use annotate_snippets::snippet::*;
//!
//! Snippet {
//!     title: Some(Annotation {
//!         label: Some("mismatched types"),
//!         id: None,
//!         annotation_type: AnnotationType::Error,
//!     }),
//!     footer: vec![],
//!     slices: vec![
//!         Slice {
//!             source: "Foo",
//!             line_start: 51,
//!             origin: Some("src/format.rs"),
//!             fold: false,
//!             annotations: vec![],
//!         },
//!         Slice {
//!             source: "Faa",
//!             line_start: 129,
//!             origin: Some("src/display.rs"),
//!             fold: false,
//!             annotations: vec![],
//!         },
//!     ],
//!     opt: Default::default(),
//! };
//! ```
use crate::display_list::FormatOptions;

/// Primary structure provided for formatting
#[derive(Debug, Default)]
pub struct Snippet<'a> {
    pub title: Option<Annotation<'a>>,
    pub footer: Vec<Annotation<'a>>,
    pub slices: Vec<Slice<'a>>,
    pub opt: FormatOptions,
}

/// Structure containing the slice of text to be annotated and
/// basic information about the location of the slice.
///
/// One `Slice` is meant to represent a single, continuous,
/// slice of source code that you want to annotate.
#[derive(Debug)]
pub struct Slice<'a> {
    pub source: &'a str,
    pub line_start: usize,
    pub origin: Option<&'a str>,
    pub annotations: Vec<SourceAnnotation<'a>>,
    /// If set explicitly to `true`, the snippet will fold
    /// parts of the slice that don't contain any annotations.
    pub fold: bool,
}

/// Types of annotations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnnotationType {
    /// Error annotations are displayed using red color and "^" character.
    Error,
    /// Warning annotations are displayed using blue color and "-" character.
    Warning,
    Info,
    Note,
    Help,
}

/// An annotation for a `Slice`.
#[derive(Debug)]
pub struct SourceAnnotation<'a> {
    pub range: (usize, usize),
    pub label: &'a str,
    pub annotation_type: AnnotationType,
}

/// An annotation for a `Snippet`.
#[derive(Debug)]
pub struct Annotation<'a> {
    /// Identifier of the annotation. Usually error code like "E0308".
    pub id: Option<&'a str>,
    pub label: Option<&'a str>,
    pub annotation_type: AnnotationType,
}
