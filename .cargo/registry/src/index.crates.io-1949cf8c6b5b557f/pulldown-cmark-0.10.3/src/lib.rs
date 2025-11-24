// Copyright 2015 Google Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Pull parser for [CommonMark](https://commonmark.org). This crate provides a [Parser](struct.Parser.html) struct
//! which is an iterator over [Event](enum.Event.html)s. This iterator can be used
//! directly, or to output HTML using the [HTML module](html/index.html).
//!
//! By default, only CommonMark features are enabled. To use extensions like tables,
//! footnotes or task lists, enable them by setting the corresponding flags in the
//! [Options](struct.Options.html) struct.
//!
//! # Example
//! ```rust
//! use pulldown_cmark::{Parser, Options};
//!
//! let markdown_input = "Hello world, this is a ~~complicated~~ *very simple* example.";
//!
//! // Set up options and parser. Strikethroughs are not part of the CommonMark standard
//! // and we therefore must enable it explicitly.
//! let mut options = Options::empty();
//! options.insert(Options::ENABLE_STRIKETHROUGH);
//! let parser = Parser::new_ext(markdown_input, options);
//!
//! # #[cfg(feature = "html")] {
//! // Write to String buffer.
//! let mut html_output = String::new();
//! pulldown_cmark::html::push_html(&mut html_output, parser);
//!
//! // Check that the output is what we expected.
//! let expected_html = "<p>Hello world, this is a <del>complicated</del> <em>very simple</em> example.</p>\n";
//! assert_eq!(expected_html, &html_output);
//! # }
//! ```
//!
//! Note that consecutive text events can happen due to the manner in which the
//! parser evaluates the source. A utility `TextMergeStream` exists to improve
//! the comfort of iterating the events:
//!
//! ```rust
//! use pulldown_cmark::{Event, Parser, TextMergeStream};
//!
//! let markdown_input = "Hello world, this is a ~~complicated~~ *very simple* example.";
//!
//! let iterator = TextMergeStream::new(Parser::new(markdown_input));
//!
//! for event in iterator {
//!     match event {
//!         Event::Text(text) => println!("{}", text),
//!         _ => {}
//!     }
//! }
//! ```
//!

// When compiled for the rustc compiler itself we want to make sure that this is
// an unstable crate.
#![cfg_attr(rustbuild, feature(staged_api, rustc_private))]
#![cfg_attr(rustbuild, unstable(feature = "rustc_private", issue = "27812"))]
// Forbid unsafe code unless the SIMD feature is enabled.
#![cfg_attr(not(feature = "simd"), forbid(unsafe_code))]
#![warn(missing_debug_implementations)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "html")]
pub mod html;

pub mod utils;

mod entities;
mod firstpass;
mod linklabel;
mod parse;
mod puncttable;
mod scanners;
mod strings;
mod tree;

use std::fmt::Display;

pub use crate::parse::{
    BrokenLink, BrokenLinkCallback, DefaultBrokenLinkCallback, OffsetIter, Parser, RefDefs,
};
pub use crate::strings::{CowStr, InlineStr};
pub use crate::utils::*;

/// Codeblock kind.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CodeBlockKind<'a> {
    Indented,
    /// The value contained in the tag describes the language of the code, which may be empty.
    #[cfg_attr(feature = "serde", serde(borrow))]
    Fenced(CowStr<'a>),
}

impl<'a> CodeBlockKind<'a> {
    pub fn is_indented(&self) -> bool {
        matches!(*self, CodeBlockKind::Indented)
    }

    pub fn is_fenced(&self) -> bool {
        matches!(*self, CodeBlockKind::Fenced(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MetadataBlockKind {
    YamlStyle,
    PlusesStyle,
}

/// Tags for elements that can contain other elements.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Tag<'a> {
    /// A paragraph of text and other inline elements.
    Paragraph,

    /// A heading, with optional identifier, classes and custom attributes.
    /// The identifier is prefixed with `#` and the last one in the attributes
    /// list is chosen, classes are prefixed with `.` and custom attributes
    /// have no prefix and can optionally have a value (`myattr` or `myattr=myvalue`).
    Heading {
        level: HeadingLevel,
        id: Option<CowStr<'a>>,
        classes: Vec<CowStr<'a>>,
        /// The first item of the tuple is the attr and second one the value.
        attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
    },

    BlockQuote,
    /// A code block.
    CodeBlock(CodeBlockKind<'a>),

    /// A HTML block.
    HtmlBlock,

    /// A list. If the list is ordered the field indicates the number of the first item.
    /// Contains only list items.
    List(Option<u64>), // TODO: add delim and tight for ast (not needed for html)
    /// A list item.
    Item,
    /// A footnote definition. The value contained is the footnote's label by which it can
    /// be referred to.
    #[cfg_attr(feature = "serde", serde(borrow))]
    FootnoteDefinition(CowStr<'a>),

    /// A table. Contains a vector describing the text-alignment for each of its columns.
    Table(Vec<Alignment>),
    /// A table header. Contains only `TableCell`s. Note that the table body starts immediately
    /// after the closure of the `TableHead` tag. There is no `TableBody` tag.
    TableHead,
    /// A table row. Is used both for header rows as body rows. Contains only `TableCell`s.
    TableRow,
    TableCell,

    // span-level tags
    Emphasis,
    Strong,
    Strikethrough,

    /// A link.
    Link {
        link_type: LinkType,
        dest_url: CowStr<'a>,
        title: CowStr<'a>,
        /// Identifier of reference links, e.g. `world` in the link `[hello][world]`.
        id: CowStr<'a>,
    },

    /// An image. The first field is the link type, the second the destination URL and the third is a title,
    /// the fourth is the link identifier.
    Image {
        link_type: LinkType,
        dest_url: CowStr<'a>,
        title: CowStr<'a>,
        /// Identifier of reference links, e.g. `world` in the link `[hello][world]`.
        id: CowStr<'a>,
    },

    /// A metadata block.
    MetadataBlock(MetadataBlockKind),
}

impl<'a> Tag<'a> {
    pub fn to_end(&self) -> TagEnd {
        match self {
            Tag::Paragraph => TagEnd::Paragraph,
            Tag::Heading { level, .. } => TagEnd::Heading(*level),
            Tag::BlockQuote => TagEnd::BlockQuote,
            Tag::CodeBlock(_) => TagEnd::CodeBlock,
            Tag::HtmlBlock => TagEnd::HtmlBlock,
            Tag::List(number) => TagEnd::List(number.is_some()),
            Tag::Item => TagEnd::Item,
            Tag::FootnoteDefinition(_) => TagEnd::FootnoteDefinition,
            Tag::Table(_) => TagEnd::Table,
            Tag::TableHead => TagEnd::TableHead,
            Tag::TableRow => TagEnd::TableRow,
            Tag::TableCell => TagEnd::TableCell,
            Tag::Emphasis => TagEnd::Emphasis,
            Tag::Strong => TagEnd::Strong,
            Tag::Strikethrough => TagEnd::Strikethrough,
            Tag::Link { .. } => TagEnd::Link,
            Tag::Image { .. } => TagEnd::Image,
            Tag::MetadataBlock(kind) => TagEnd::MetadataBlock(*kind),
        }
    }
}

/// The end of a `Tag`.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TagEnd {
    Paragraph,
    Heading(HeadingLevel),

    BlockQuote,
    CodeBlock,

    HtmlBlock,

    /// A list, `true` for ordered lists.
    List(bool),
    Item,
    FootnoteDefinition,

    Table,
    TableHead,
    TableRow,
    TableCell,

    Emphasis,
    Strong,
    Strikethrough,

    Link,
    Image,

    MetadataBlock(MetadataBlockKind),
}

impl<'a> From<Tag<'a>> for TagEnd {
    fn from(value: Tag) -> Self {
        value.to_end()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum HeadingLevel {
    H1 = 1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl Display for HeadingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::H1 => write!(f, "h1"),
            Self::H2 => write!(f, "h2"),
            Self::H3 => write!(f, "h3"),
            Self::H4 => write!(f, "h4"),
            Self::H5 => write!(f, "h5"),
            Self::H6 => write!(f, "h6"),
        }
    }
}

/// Returned when trying to convert a `usize` into a `Heading` but it fails
/// because the usize isn't a valid heading level
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct InvalidHeadingLevel(usize);

impl TryFrom<usize> for HeadingLevel {
    type Error = InvalidHeadingLevel;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::H1),
            2 => Ok(Self::H2),
            3 => Ok(Self::H3),
            4 => Ok(Self::H4),
            5 => Ok(Self::H5),
            6 => Ok(Self::H6),
            _ => Err(InvalidHeadingLevel(value)),
        }
    }
}

/// Type specifier for inline links. See [the Tag::Link](enum.Tag.html#variant.Link) for more information.
#[derive(Clone, Debug, PartialEq, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LinkType {
    /// Inline link like `[foo](bar)`
    Inline,
    /// Reference link like `[foo][bar]`
    Reference,
    /// Reference without destination in the document, but resolved by the broken_link_callback
    ReferenceUnknown,
    /// Collapsed link like `[foo][]`
    Collapsed,
    /// Collapsed link without destination in the document, but resolved by the broken_link_callback
    CollapsedUnknown,
    /// Shortcut link like `[foo]`
    Shortcut,
    /// Shortcut without destination in the document, but resolved by the broken_link_callback
    ShortcutUnknown,
    /// Autolink like `<http://foo.bar/baz>`
    Autolink,
    /// Email address in autolink like `<john@example.org>`
    Email,
}

impl LinkType {
    /// Map the link type to an equivalent _Unknown link type.
    fn to_unknown(self) -> Self {
        match self {
            LinkType::Reference => LinkType::ReferenceUnknown,
            LinkType::Collapsed => LinkType::CollapsedUnknown,
            LinkType::Shortcut => LinkType::ShortcutUnknown,
            _ => unreachable!(),
        }
    }
}

/// Markdown events that are generated in a preorder traversal of the document
/// tree, with additional `End` events whenever all of an inner node's children
/// have been visited.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Event<'a> {
    /// Start of a tagged element. Events that are yielded after this event
    /// and before its corresponding `End` event are inside this element.
    /// Start and end events are guaranteed to be balanced.
    #[cfg_attr(feature = "serde", serde(borrow))]
    Start(Tag<'a>),
    /// End of a tagged element.
    End(TagEnd),
    /// A text node.
    #[cfg_attr(feature = "serde", serde(borrow))]
    Text(CowStr<'a>),
    /// An inline code node.
    #[cfg_attr(feature = "serde", serde(borrow))]
    Code(CowStr<'a>),
    /// An HTML node.
    #[cfg_attr(feature = "serde", serde(borrow))]
    Html(CowStr<'a>),
    /// An inline HTML node.
    #[cfg_attr(feature = "serde", serde(borrow))]
    InlineHtml(CowStr<'a>),
    /// A reference to a footnote with given label, which may or may not be defined
    /// by an event with a `Tag::FootnoteDefinition` tag. Definitions and references to them may
    /// occur in any order.
    #[cfg_attr(feature = "serde", serde(borrow))]
    FootnoteReference(CowStr<'a>),
    /// A soft line break.
    SoftBreak,
    /// A hard line break.
    HardBreak,
    /// A horizontal ruler.
    Rule,
    /// A task list marker, rendered as a checkbox in HTML. Contains a true when it is checked.
    TaskListMarker(bool),
}

/// Table column text alignment.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]

pub enum Alignment {
    /// Default text alignment.
    None,
    Left,
    Center,
    Right,
}

bitflags::bitflags! {
    /// Option struct containing flags for enabling extra features
    /// that are not part of the CommonMark spec.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Options: u32 {
        const ENABLE_TABLES = 1 << 1;
        /// GitHub-compatible footnote syntax.
        ///
        /// Footnotes are referenced with the syntax `[^IDENT]`,
        /// and defined with an identifier followed by a colon at top level.
        ///
        /// ---
        ///
        /// ```markdown
        /// Footnote referenced [^1].
        ///
        /// [^1]: footnote defined
        /// ```
        ///
        /// Footnote referenced [^1].
        ///
        /// [^1]: footnote defined
        const ENABLE_FOOTNOTES = 1 << 2;
        const ENABLE_STRIKETHROUGH = 1 << 3;
        const ENABLE_TASKLISTS = 1 << 4;
        const ENABLE_SMART_PUNCTUATION = 1 << 5;
        /// Extension to allow headings to have ID and classes.
        ///
        /// `# text { #id .class1 .class2 myattr other_attr=myvalue }`
        /// is interpreted as a level 1 heading
        /// with the content `text`, ID `id`, classes `class1` and `class2` and
        /// custom attributes `myattr` (without value) and
        /// `other_attr` with value `myvalue`.
        /// Note that ID, classes, and custom attributes should be space-separated.
        const ENABLE_HEADING_ATTRIBUTES = 1 << 6;
        /// Metadata blocks in YAML style, i.e.:
        /// - starting with a `---` line
        /// - ending with a `---` or `...` line
        const ENABLE_YAML_STYLE_METADATA_BLOCKS = 1 << 7;
        /// Metadata blocks delimited by:
        /// - `+++` line at start
        /// - `+++` line at end
        const ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS = 1 << 8;
        /// Older footnote syntax. This flag implies `ENABLE_FOOTNOTES`, changing it to use an
        /// older syntax instead of the new, default, GitHub-compatible syntax.
        ///
        /// New syntax is different from the old syntax regarding
        /// indentation, nesting, and footnote references with no definition:
        ///
        /// ```markdown
        /// [^1]: In new syntax, this is two footnote definitions.
        /// [^2]: In old syntax, this is a single footnote definition with two lines.
        ///
        /// [^3]:
        ///
        ///     In new syntax, this is a footnote with two paragraphs.
        ///
        ///     In old syntax, this is a footnote followed by a code block.
        ///
        /// In new syntax, this undefined footnote definition renders as
        /// literal text [^4]. In old syntax, it creates a dangling link.
        /// ```
        const ENABLE_OLD_FOOTNOTES = (1 << 9) | (1 << 2);
    }
}

impl Options {
    pub(crate) fn has_gfm_footnotes(&self) -> bool {
        self.contains(Options::ENABLE_FOOTNOTES) && !self.contains(Options::ENABLE_OLD_FOOTNOTES)
    }
}
