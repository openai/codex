//! Index Markdown ancestry in the same logical byte offsets as the transcript caret.
//! Section ranges share the index. They are added after the parser's blocks so marking
//! a caret still finds a block, while expansion can opt into heading-aware navigation.

use super::TextLayout;
use crate::markdown_copy::block::Kind;
use pulldown_cmark::HeadingLevel;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

pub(in crate::transcript_view) struct BlockRange {
    pub(in crate::transcript_view) parent: Option<usize>,
    pub(in crate::transcript_view) range: Range<usize>,
    pub(in crate::transcript_view) outer_prefix: usize,
    /// A section points to the parser block for its title.
    pub(in crate::transcript_view) heading: Option<usize>,
    kind: Kind,
    section: Option<usize>,
}

impl BlockRange {
    pub(in crate::transcript_view) fn label(&self) -> &'static str {
        if self.heading.is_some() {
            "section"
        } else {
            self.kind.label()
        }
    }

    pub(in crate::transcript_view) fn expand_to(&self) -> Option<usize> {
        self.section.or(self.parent)
    }
}

pub(super) struct BlockIndex {
    ranges: Vec<BlockRange>,
    parser_blocks: usize,
}

impl TextLayout {
    pub(in crate::transcript_view) fn blocks(&self) -> &[BlockRange] {
        &self.block_index().ranges
    }

    fn block_index(&self) -> &BlockIndex {
        self.blocks.get_or_init(|| {
            let mut ids = HashMap::new();
            let mut blocks: Vec<BlockRange> = Vec::new();
            let mut path = Vec::new();
            for (start, source) in self.logical_sources() {
                let Some(copy) = &source.copy else { continue };
                // Empty items carry their visible marker in the prefix.
                if copy.omit || source.range.is_empty() && !copy.code && !copy.has_list_marker() {
                    continue;
                }
                let mut node = copy.block.as_ref();
                while let Some(block) = node {
                    path.push(block);
                    node = block.parent.as_ref();
                }
                let mut parent = None;
                while let Some(block) = path.pop() {
                    let id = *ids.entry(Arc::as_ptr(block)).or_insert_with(|| {
                        blocks.push(BlockRange {
                            parent,
                            range: start..start,
                            outer_prefix: block.outer_prefix,
                            heading: None,
                            kind: block.kind,
                            section: None,
                        });
                        blocks.len() - 1
                    });
                    blocks[id].range.end = start + source.range.len();
                    parent = Some(id);
                }
            }
            let parser_blocks = blocks.len();
            // Open headings are tracked per parser container. A nested quote or list
            // item can start its own sections without cutting off the outer section.
            let mut open: HashMap<Option<usize>, Vec<(HeadingLevel, usize)>> = HashMap::new();
            for id in 0..parser_blocks {
                let container = blocks[id].parent;
                if let Kind::Heading(level) = blocks[id].kind {
                    let sections = open.entry(container).or_default();
                    while let Some(&(rank, section)) = sections.last()
                        && rank >= level
                    {
                        // A standalone rule can appear between blocks without a parser
                        // block of its own. Keep it and drop only the trailing breaks.
                        let end = self.text()[..blocks[id].range.start]
                            .trim_end_matches('\n')
                            .len();
                        blocks[section].range.end = blocks[section].range.end.max(end);
                        sections.pop();
                    }
                    let section = blocks.len();
                    blocks.push(BlockRange {
                        parent: sections.last().map(|(_, id)| *id).or(container),
                        range: blocks[id].range.clone(),
                        outer_prefix: blocks[id].outer_prefix,
                        heading: Some(id),
                        kind: blocks[id].kind,
                        section: None,
                    });
                    sections.push((level, section));
                }
                if let Some(sections) = open.get(&container) {
                    blocks[id].section = sections.last().map(|(_, id)| *id);
                    for &(_, section) in sections {
                        blocks[section].range.end = blocks[id].range.end;
                    }
                }
            }
            for (container, sections) in open {
                let end = container.map_or_else(
                    || self.text().trim_end_matches('\n').len(),
                    |id| blocks[id].range.end,
                );
                for (_, section) in sections {
                    blocks[section].range.end = blocks[section].range.end.max(end);
                }
            }
            BlockIndex {
                ranges: blocks,
                parser_blocks,
            }
        })
    }

    pub(in crate::transcript_view) fn block_at(&self, offset: usize) -> Option<usize> {
        let index = self.block_index();
        let blocks = &index.ranges[..index.parser_blocks];
        let mut at = blocks
            .partition_point(|b| b.range.start <= offset)
            .checked_sub(1)?;
        // The caret on trailing table padding can sit at the exclusive end.
        while offset > blocks[at].range.end {
            at = blocks[at].parent?;
        }
        Some(at)
    }
}
