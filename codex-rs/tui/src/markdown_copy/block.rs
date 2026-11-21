//! Block ancestry shared by logical lines of a render. Pointer identity distinguishes
//! adjacent blocks with identical contents. Wrapping shares the original annotations.

use pulldown_cmark::HeadingLevel;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) parent: Option<Arc<Block>>,
    pub(crate) kind: Kind,
    /// Bytes of container syntax to drop when copying this block on its own.
    pub(crate) outer_prefix: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Paragraph,
    Heading(HeadingLevel),
    Code,
    Quote,
    List,
    Item,
    Table,
}

impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Heading(_) => "heading",
            Self::Code => "code block",
            Self::Quote => "blockquote",
            Self::List => "list",
            Self::Item => "list item",
            Self::Table => "table",
        }
    }
}

#[derive(Default)]
pub(crate) struct Stack {
    current: Option<Arc<Block>>,
    depth: usize,
}

impl Stack {
    pub(crate) fn current(&self) -> Option<Arc<Block>> {
        self.current.clone()
    }

    pub(crate) fn kind(tag: &Tag<'_>) -> Option<Kind> {
        Some(match tag {
            Tag::Paragraph => Kind::Paragraph,
            Tag::Heading { level, .. } => Kind::Heading(*level),
            Tag::BlockQuote => Kind::Quote,
            Tag::CodeBlock(_) => Kind::Code,
            Tag::List(_) => Kind::List,
            Tag::Item => Kind::Item,
            Tag::Table(_) => Kind::Table,
            _ => return None,
        })
    }

    pub(crate) fn push(&mut self, kind: Kind, outer_prefix: usize) {
        self.depth += 1;
        if self.depth <= super::MAX_COPY_DEPTH {
            self.current = Some(Arc::new(Block {
                parent: self.current.take(),
                kind,
                outer_prefix,
            }));
        }
    }

    pub(crate) fn end(&mut self, tag: TagEnd) {
        if matches!(
            tag,
            TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::BlockQuote
                | TagEnd::CodeBlock
                | TagEnd::List(_)
                | TagEnd::Item
                | TagEnd::Table
        ) {
            if self.depth <= super::MAX_COPY_DEPTH {
                self.current = self.current.take().and_then(|block| block.parent.clone());
            }
            self.depth = self.depth.saturating_sub(1);
        }
    }
}
