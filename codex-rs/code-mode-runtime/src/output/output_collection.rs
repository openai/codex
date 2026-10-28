//! Bounded head/tail retention for raw Code Mode output. Text has a rolling
//! byte allowance. Admit whole images and audio clips in source order against
//! independent byte allowances. Item pressure can free space for later media.

use std::collections::VecDeque;

use super::CollectedOutput;
use super::OutputOmission;

use codex_code_mode_protocol::FunctionCallOutputContentItem as OutputItem;
use codex_utils_output_truncation::OutputBoundary;

pub(super) use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_AUDIO_BYTES as MAX_AUDIO_BYTES;
pub(super) use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_BYTES as MAX_OUTPUT_BYTES;
pub(super) use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_IMAGE_BYTES as MAX_IMAGE_BYTES;

/// Limit logical items independently of payload bytes. An observation that
/// fits this limit must keep its item boundaries even if our storage layout changes.
pub(crate) const MAX_OUTPUT_ITEMS: usize = 32 * 1024;
const TEXT_BLOCK_BYTES: usize = 4096;

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum RetainedItem {
    Content(OutputItem),
    Partial(Box<PartialText>),
    Gap(OutputOmission),
}

/// Store a partially evicted suffix in blocks so later writes can release
/// its oldest bytes without copying the rest. Only the first block has an offset.
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct PartialText {
    blocks: VecDeque<String>,
    front_offset: usize,
    bytes: usize,
    omitted: u64,
    continues_prefix: bool,
}

impl RetainedItem {
    fn retain(mut item: OutputItem) -> Self {
        let payload = match &mut item {
            OutputItem::InputImage { image_url, .. } => image_url,
            OutputItem::InputAudio { audio_url } => audio_url,
            OutputItem::InputText { text } if text.len() > MAX_OUTPUT_BYTES => {
                // Eviction copies only the head and suffix that can survive.
                return Self::Content(item);
            }
            OutputItem::InputText { text } => text,
        };
        payload.shrink_to_fit();
        Self::Content(item)
    }

    /// Evict bytes after the retained prefix and keep any surviving suffix.
    fn text_suffix(text: &str, prefix_len: usize, bytes_to_evict: usize) -> Self {
        let tail = &text[prefix_len..];
        let omitted = tail.ceil_char_boundary(bytes_to_evict.min(tail.len()));
        let mut suffix = &tail[omitted..];
        if suffix.is_empty() {
            return Self::Gap(OutputOmission {
                text_bytes: omitted as u64,
                text_items: 1,
                ..OutputOmission::default()
            });
        }
        let bytes = suffix.len();
        let mut blocks = VecDeque::new();
        while !suffix.is_empty() {
            let end = suffix.floor_char_boundary(suffix.len().min(TEXT_BLOCK_BYTES));
            blocks.push_back(suffix[..end].to_owned());
            suffix = &suffix[end..];
        }
        Self::Partial(Box::new(PartialText {
            blocks,
            front_offset: 0,
            bytes,
            omitted: omitted as u64,
            continues_prefix: prefix_len > 0,
        }))
    }

    fn content_bytes(&self) -> usize {
        match self {
            Self::Content(OutputItem::InputText { text }) => text.len(),
            Self::Partial(text) => text.bytes,
            Self::Content(OutputItem::InputImage { .. } | OutputItem::InputAudio { .. })
            | Self::Gap(_) => 0,
        }
    }

    fn omission(&self) -> OutputOmission {
        let mut loss = OutputOmission::default();
        match self {
            Self::Content(OutputItem::InputText { text }) => {
                loss.text_bytes = text.len() as u64;
                loss.text_items = 1;
            }
            Self::Content(OutputItem::InputImage { .. }) => loss.image_items = 1,
            Self::Content(OutputItem::InputAudio { .. }) => loss.audio_items = 1,
            Self::Partial(text) => {
                loss.text_bytes = text.omitted.saturating_add(text.bytes as u64);
                loss.text_items = 1;
            }
            Self::Gap(omitted) => loss = *omitted,
        }
        loss
    }

    fn move_into(self, output: &mut CollectedOutput) {
        match self {
            Self::Content(item) => output.items.push(item),
            Self::Gap(loss) => output.push_gap(
                OutputBoundary {
                    item_index: output.items.len(),
                    text_offset: 0,
                },
                loss,
            ),
            Self::Partial(text) => {
                // A split suffix continues the preceding prefix. A suffix whose
                // prefix was discarded starts its own logical item.
                if !text.continues_prefix {
                    output.items.push(OutputItem::InputText {
                        text: String::new(),
                    });
                }
                let item_index = output.items.len() - 1;
                let OutputItem::InputText { text: retained } = &mut output.items[item_index] else {
                    unreachable!("a split suffix follows its text prefix");
                };
                let text_offset = retained.len();
                retained.reserve_exact(text.bytes);
                for (i, block) in text.blocks.into_iter().enumerate() {
                    let start = if i == 0 { text.front_offset } else { 0 };
                    retained.push_str(&block[start..]);
                }
                if text.omitted > 0 {
                    output.push_gap(
                        OutputBoundary {
                            item_index,
                            text_offset,
                        },
                        OutputOmission {
                            text_bytes: text.omitted,
                            ..OutputOmission::default()
                        },
                    );
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
struct OpenOutput {
    items: VecDeque<RetainedItem>,
    content_bytes: usize,
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct BoundedOutput {
    state: Retention,
    media: MediaBytes,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
struct MediaBytes {
    images: usize,
    audio: usize,
}

impl MediaBytes {
    fn admit(&mut self, item: &OutputItem) -> Result<(), OutputOmission> {
        let (len, used, limit, omission) = match item {
            OutputItem::InputText { .. } => return Ok(()),
            OutputItem::InputImage { image_url, .. } => (
                image_url.len(),
                &mut self.images,
                MAX_IMAGE_BYTES,
                OutputOmission {
                    image_items: 1,
                    ..OutputOmission::default()
                },
            ),
            OutputItem::InputAudio { audio_url } => (
                audio_url.len(),
                &mut self.audio,
                MAX_AUDIO_BYTES,
                OutputOmission {
                    audio_items: 1,
                    ..OutputOmission::default()
                },
            ),
        };
        if len > limit - *used {
            return Err(omission);
        }
        *used += len;
        Ok(())
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum Retention {
    /// All individually fitting items are intact, in source order.
    Open(OpenOutput),
    /// A fixed prefix and a rolling tail with omissions in source order.
    HeadTail(HeadTailOutput),
}

impl Default for BoundedOutput {
    fn default() -> Self {
        Self {
            state: Retention::Open(OpenOutput::default()),
            media: MediaBytes::default(),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct HeadTailOutput {
    head: Vec<RetainedItem>,
    head_bytes: usize,
    tail: VecDeque<RetainedItem>,
    // The first tail item that might still have text bytes.
    cursor: usize,
    // Adjacent tail gaps occupy one logical item. Repack their physical slots in batches.
    adjacent_gaps: usize,
    // Text bytes retained in the entire observation, including the head.
    content_bytes: usize,
    // Gaps at the front of the tail no longer need their own slots.
    middle: OutputOmission,
}

impl HeadTailOutput {
    fn from_open(open: OpenOutput) -> Self {
        let mut output = Self {
            head: Vec::new(),
            head_bytes: 0,
            tail: open.items,
            cursor: 0,
            adjacent_gaps: 0,
            content_bytes: open.content_bytes,
            middle: OutputOmission::default(),
        };
        while output.head.len() < MAX_OUTPUT_ITEMS / 2 {
            let Some(item) = output.tail.pop_front() else {
                break;
            };
            let bytes = item.content_bytes();
            let remaining = MAX_OUTPUT_BYTES / 2 - output.head_bytes;
            if bytes <= remaining {
                output.head_bytes += bytes;
                output.head.push(item);
            } else if output.content_bytes <= MAX_OUTPUT_BYTES {
                // Under item pressure a whole item may borrow unused tail bytes.
                output.head_bytes += bytes;
                output.head.push(item);
                break;
            } else {
                output.tail.push_front(item);
                output.split_front(remaining);
                break;
            }
        }
        output
    }

    /// Preserve a text prefix and copy only suffix bytes that could still fit.
    fn split_front(&mut self, remaining: usize) {
        let Some(RetainedItem::Content(OutputItem::InputText { text })) = self.tail.front() else {
            return;
        };
        let split = text.floor_char_boundary(remaining);
        if split == 0 {
            return;
        }
        self.head.push(RetainedItem::Content(OutputItem::InputText {
            text: text[..split].to_owned(),
        }));
        self.head_bytes += split;
        let excess = self.content_bytes - MAX_OUTPUT_BYTES;
        let suffix = RetainedItem::text_suffix(text, split, excess);
        self.content_bytes -= text.len() - split - suffix.content_bytes();
        if matches!(suffix, RetainedItem::Gap(_))
            && matches!(self.tail.get(1), Some(RetainedItem::Gap(_)))
        {
            self.adjacent_gaps += 1;
        }
        self.tail[0] = suffix;
    }

    fn advance_front(&mut self) -> RetainedItem {
        self.cursor = self.cursor.saturating_sub(1);
        let Some(item) = self.tail.pop_front() else {
            unreachable!("eviction requires a nonempty tail");
        };
        if matches!(item, RetainedItem::Gap(_))
            && matches!(self.tail.front(), Some(RetainedItem::Gap(_)))
        {
            self.adjacent_gaps -= 1;
        }
        item
    }

    fn trim(&mut self, media: &mut MediaBytes) {
        if self.content_bytes > MAX_OUTPUT_BYTES && self.head_bytes > MAX_OUTPUT_BYTES / 2 {
            // Only the last prefix item can have borrowed space. Reconsider it
            // before the middle gap, whose source position follows that item.
            let Some(item) = self.head.pop() else {
                unreachable!("borrowed bytes require a prefix item");
            };
            self.head_bytes -= item.content_bytes();
            if self.middle != OutputOmission::default() {
                self.tail
                    .push_front(RetainedItem::Gap(std::mem::take(&mut self.middle)));
            }
            self.tail.push_front(item);
            self.cursor = 0;
            let remaining = MAX_OUTPUT_BYTES / 2 - self.head_bytes;
            self.split_front(remaining);
        }
        // Skip retained media and preserve each gap between its source neighbors.
        let used = &mut self.content_bytes;
        let cursor = &mut self.cursor;
        while *used > MAX_OUTPUT_BYTES {
            let slot = &mut self.tail[*cursor];
            let old_bytes = slot.content_bytes();
            if old_bytes > 0 {
                let excess = *used - MAX_OUTPUT_BYTES;
                let remaining = old_bytes.saturating_sub(excess);
                match slot {
                    RetainedItem::Content(OutputItem::InputText { text }) if remaining > 0 => {
                        *slot = RetainedItem::text_suffix(text, /*prefix_len*/ 0, excess);
                    }
                    RetainedItem::Partial(text) if remaining > 0 => {
                        // Later writes may evict more of this same suffix.
                        while text.bytes > remaining {
                            let front = &text.blocks[0][text.front_offset..];
                            let removed =
                                front.ceil_char_boundary((text.bytes - remaining).min(front.len()));
                            text.bytes -= removed;
                            text.omitted = text.omitted.saturating_add(removed as u64);
                            if removed == front.len() {
                                text.blocks.pop_front();
                                text.front_offset = 0;
                            } else {
                                text.front_offset += removed;
                            }
                        }
                        if text.bytes == 0 {
                            *slot = RetainedItem::Gap(slot.omission());
                        }
                    }
                    _ => *slot = RetainedItem::Gap(slot.omission()),
                }
                let new_bytes = slot.content_bytes();
                *used -= old_bytes - new_bytes;
                if new_bytes > 0 {
                    break;
                }
                self.adjacent_gaps += usize::from(
                    *cursor > 0 && matches!(self.tail[*cursor - 1], RetainedItem::Gap(_)),
                ) + usize::from(matches!(
                    self.tail.get(*cursor + 1),
                    Some(RetainedItem::Gap(_))
                ));
            }
            *cursor += 1;
        }
        // Each sweep removes at least this many slots. Tiny writes cannot
        // repeatedly scan the tail, even when a live image brackets many gaps.
        if self.adjacent_gaps >= MAX_OUTPUT_ITEMS.div_ceil(8) {
            let slots = self.tail.make_contiguous();
            for i in 1..slots.len() {
                let (prior, rest) = slots.split_at_mut(i);
                if let (RetainedItem::Gap(left), RetainedItem::Gap(right)) =
                    (&mut prior[i - 1], &mut rest[0])
                {
                    right.combine(std::mem::take(left));
                }
            }
            self.tail.retain(|item| !matches!(item, RetainedItem::Gap(loss) if *loss == OutputOmission::default()));
            self.cursor = 0;
            self.adjacent_gaps = 0;
        }
        let split_suffix = if matches!(self.tail.front(), Some(RetainedItem::Partial(text)) if text.continues_prefix)
        {
            // The suffix shares its head item's slot, and no middle gap exists yet.
            if self.head.len() + self.tail.len() - self.adjacent_gaps <= MAX_OUTPUT_ITEMS + 1 {
                return;
            }
            Some(self.advance_front())
        } else {
            None
        };
        // A middle gap costs a slot only once it exists. The first eviction
        // creates that gap, so recompute the logical item count on each pass.
        while self.head.len() + self.tail.len() - self.adjacent_gaps
            + usize::from(self.middle != OutputOmission::default())
            > MAX_OUTPUT_ITEMS
        {
            let item = self.advance_front();
            self.content_bytes -= item.content_bytes();
            match &item {
                RetainedItem::Content(OutputItem::InputImage { image_url, .. }) => {
                    media.images -= image_url.len();
                }
                RetainedItem::Content(OutputItem::InputAudio { audio_url }) => {
                    media.audio -= audio_url.len();
                }
                _ => {}
            }
            self.middle.combine(item.omission());
        }
        while matches!(self.tail.front(), Some(RetainedItem::Gap(_))) {
            let item = self.advance_front();
            self.middle.combine(item.omission());
        }
        if let Some(suffix) = split_suffix {
            self.tail
                .push_front(RetainedItem::Gap(std::mem::take(&mut self.middle)));
            self.tail.push_front(suffix);
            self.cursor = 0;
        }
    }
}

impl BoundedOutput {
    /// A completed error is one logical text item, including its label. Do not
    /// allocate an arbitrarily large formatted copy before selecting its output.
    pub(crate) fn push_error(&mut self, error: &str) {
        const LABEL: &str = "Script error:\n";
        // No selection can retain more than MAX_OUTPUT_BYTES from either end
        // of this item. Keep both possible spans until push() decides how the
        // existing head, tail and item pressure affect them. Round outward so
        // a UTF-8 boundary cannot pull bytes from the prefix across the gap.
        let head_end = error.ceil_char_boundary(error.len().min(MAX_OUTPUT_BYTES - LABEL.len()));
        let tail_start =
            error.floor_char_boundary(error.len().saturating_sub(MAX_OUTPUT_BYTES).max(head_end));
        self.push(OutputItem::InputText {
            text: format!("{LABEL}{}{}", &error[..head_end], &error[tail_start..]),
        });
        let skipped = tail_start - head_end;
        if skipped > 0 {
            // This item alone exceeds the content budget. Its newest, retained
            // suffix owns the gap, even when an image separates it from older loss.
            let Retention::HeadTail(output) = &mut self.state else {
                unreachable!("an oversized error must enter head/tail retention");
            };
            let Some(RetainedItem::Partial(text)) = output.tail.back_mut() else {
                unreachable!("the tail retains the end of the oversized error");
            };
            text.omitted = text.omitted.saturating_add(skipped as u64);
        }
    }

    pub(crate) fn push(&mut self, item: OutputItem) {
        if let Err(loss) = self.media.admit(&item) {
            self.push_retained(RetainedItem::Gap(loss));
            // Adding the loss marker can evict earlier media from a full observation.
            // Replace only this item's marker if that freed enough space.
            if let Retention::HeadTail(split) = &mut self.state
                && let Some(slot) = split.tail.back_mut()
                && matches!(slot, RetainedItem::Gap(omitted) if *omitted == loss)
                && self.media.admit(&item).is_ok()
            {
                *slot = RetainedItem::retain(item);
            }
            return;
        }
        self.push_retained(RetainedItem::retain(item));
    }

    fn push_retained(&mut self, item: RetainedItem) {
        let bytes = item.content_bytes();
        let (tail, retained) = match &mut self.state {
            Retention::Open(open) => (&mut open.items, &mut open.content_bytes),
            Retention::HeadTail(split) => (&mut split.tail, &mut split.content_bytes),
        };
        if let RetainedItem::Gap(loss) = &item
            && let Some(RetainedItem::Gap(last)) = tail.back_mut()
        {
            last.combine(*loss);
            return;
        }
        tail.push_back(item);
        *retained += bytes;
        if let Retention::Open(open) = &mut self.state {
            if open.content_bytes <= MAX_OUTPUT_BYTES && open.items.len() <= MAX_OUTPUT_ITEMS {
                return;
            }
            self.state = Retention::HeadTail(HeadTailOutput::from_open(std::mem::take(open)));
        }
        if let Retention::HeadTail(split) = &mut self.state {
            split.trim(&mut self.media);
        }
    }

    fn collect(items: impl Iterator<Item = RetainedItem>) -> CollectedOutput {
        let mut output = CollectedOutput::default();
        for item in items {
            item.move_into(&mut output);
        }
        output
    }

    pub(crate) fn into_collected_output(self) -> CollectedOutput {
        match self.state {
            Retention::Open(open) => Self::collect(open.items.into_iter()),
            Retention::HeadTail(split) => Self::collect(
                split
                    .head
                    .into_iter()
                    .chain(
                        (split.middle != OutputOmission::default())
                            .then_some(RetainedItem::Gap(split.middle)),
                    )
                    .chain(split.tail),
            ),
        }
    }
}

#[cfg(test)]
#[path = "output_collection_tests.rs"]
mod tests;
