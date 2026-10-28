//! Regression coverage for bounded output retention.

#![cfg(test)]

use super::*;
use crate::output::OutputCollector;
use codex_code_mode_protocol::CodeModeOutputTruncation;
use codex_code_mode_protocol::ImageDetail;
use pretty_assertions::assert_eq;

fn text(value: &str) -> OutputItem {
    OutputItem::InputText {
        text: value.to_owned(),
    }
}

fn image(image_url: String) -> OutputItem {
    OutputItem::InputImage {
        image_url,
        detail: Some(ImageDetail::Original),
    }
}

fn audio(audio_url: String) -> OutputItem {
    OutputItem::InputAudio { audio_url }
}

fn collect(items: impl IntoIterator<Item = OutputItem>) -> BoundedOutput {
    let mut output = BoundedOutput::default();
    for item in items {
        output.push(item);
    }
    output
}

fn retained_capacity(collector: &BoundedOutput) -> usize {
    let capacity = |item: &RetainedItem| match item {
        RetainedItem::Content(OutputItem::InputText { text }) => text.capacity(),
        RetainedItem::Content(OutputItem::InputImage { image_url, .. }) => image_url.capacity(),
        RetainedItem::Content(OutputItem::InputAudio { audio_url }) => audio_url.capacity(),
        RetainedItem::Gap(_) => 0,
        RetainedItem::Partial(text) => text.blocks.iter().map(String::capacity).sum(),
    };
    match &collector.state {
        Retention::Open(open) => open.items.iter().map(capacity).sum(),
        Retention::HeadTail(split) => split.head.iter().chain(&split.tail).map(capacity).sum(),
    }
}

#[test]
fn all_32k_empty_writes_fit_before_the_item_limit_creates_a_gap() {
    let mut output = collect((0..32_768).map(|_| text("")));
    assert!(output.clone().into_collected_output() == vec![text(""); 32_768].into());

    output.push(text("last"));
    let snapshot = output.into_collected_output();
    assert_eq!(snapshot.items.len(), 32_767);
    assert_eq!(snapshot.items.last(), Some(&text("last")));
    let [gap] = snapshot.gaps.as_slice() else {
        panic!("expected one item-limit gap")
    };
    assert_eq!(gap.boundary.item_index, 16_384);
    assert_eq!(gap.omitted.text_items, 2);
}

#[test]
fn item_pressure_keeps_a_split_head_items_suffix() {
    let half = "x".repeat(MAX_OUTPUT_BYTES / 2);
    let retained = text(&half.repeat(2));
    let mut output = collect([text(&format!("{half}y{half}"))]);
    for _ in 0..MAX_OUTPUT_ITEMS - 1 {
        output.push(text(""));
    }
    assert!(output.clone().into_collected_output().items.first() == Some(&retained));

    output.push(text(""));
    let snapshot = output.into_collected_output();
    assert!(snapshot.items.first() == Some(&retained));
    assert_eq!(snapshot.items.len(), MAX_OUTPUT_ITEMS - 1);
    assert_eq!(snapshot.gaps.len(), 2);
    assert_eq!(snapshot.gaps[1].boundary.item_index, 1);
    assert_eq!(snapshot.gaps[1].omitted.text_items, 2);
}

#[test]
fn fitting_items_release_spare_capacity() {
    let items = [text("first"), image("image".into()), audio("audio".into())];
    let output = collect(items.iter().cloned().map(|mut item| {
        let payload = match &mut item {
            OutputItem::InputText { text } => text,
            OutputItem::InputImage { image_url, .. } => image_url,
            OutputItem::InputAudio { audio_url } => audio_url,
        };
        payload.reserve(2 * MAX_OUTPUT_BYTES);
        item
    }));
    assert!(retained_capacity(&output) <= MAX_OUTPUT_BYTES);
    assert_eq!(output.into_collected_output(), items.to_vec().into());
}

#[test]
fn rolling_unicode_suffix_survives_tiny_writes_without_unbounded_backing() {
    let half = MAX_OUTPUT_BYTES / 2;
    let original = format!("{}{}z", "a".repeat(half - 1), "😀".repeat(half));
    let mut output = collect([text(&original)]);
    let mut last_block = None;
    for _ in 0..10_001 {
        output.push(text("z"));
        let Retention::HeadTail(split) = &output.state else {
            panic!("expected truncated output")
        };
        let Some(RetainedItem::Partial(front)) = split.tail.front() else {
            panic!("expected a partial suffix")
        };
        let ptr = front.blocks.back().unwrap().as_ptr();
        assert_eq!(*last_block.get_or_insert(ptr), ptr);
        assert!(retained_capacity(&output) < MAX_OUTPUT_BYTES + TEXT_BLOCK_BYTES);
    }
    let head = original.floor_char_boundary(half);
    let tail = original.ceil_char_boundary(original.len() - (MAX_OUTPUT_BYTES - head - 10_001));
    let snapshot = output.into_collected_output();
    assert_eq!(snapshot.items.len(), 10_002);
    assert!(snapshot.items[0] == text(&[&original[..head], &original[tail..]].concat()));
    let [gap] = snapshot.gaps.as_slice() else {
        panic!("expected one split")
    };
    assert_eq!(
        gap.boundary,
        OutputBoundary {
            item_index: 0,
            text_offset: head
        }
    );
    assert_eq!(gap.omitted.text_bytes, (tail - head) as u64);
}

#[test]
fn byte_pressure_reclaims_space_borrowed_by_the_first_item() {
    let half = MAX_OUTPUT_BYTES / 2;
    let suffix = text(&"z".repeat(half));
    let first = text(&"a".repeat(half + 8));
    let mut output = collect([first.clone()]);
    for _ in 0..MAX_OUTPUT_ITEMS {
        output.push(text(""));
    }
    assert!(output.clone().into_collected_output().items[0] == first);

    output.push(suffix.clone());
    let snapshot = output.into_collected_output();
    assert_eq!(snapshot.items.last(), Some(&suffix));
    let [gap] = snapshot.gaps.as_slice() else {
        panic!("expected one middle gap")
    };
    assert!(snapshot.items[0] == text(&"a".repeat(half)));
    assert_eq!(gap.boundary.item_index, 1);
    assert_eq!(gap.omitted.text_bytes, 8);
}

#[test]
fn image_admission_uses_remaining_bytes_and_reclaims_them_on_eviction() {
    let half = MAX_OUTPUT_ITEMS / 2;
    let mut output = collect((0..half).map(|_| text("")));
    let first = image("i".repeat(MAX_IMAGE_BYTES - 2));
    let last = image("zz".into());
    for item in [first.clone(), image("xxx".into()), last.clone()] {
        output.push(item);
    }
    let snapshot = output.clone().into_collected_output();
    assert!(snapshot.items.ends_with(&[first, last]));
    let [gap] = snapshot.gaps.as_slice() else {
        panic!("expected one rejected image")
    };
    assert_eq!(gap.boundary.item_index, half + 1);
    assert_eq!(gap.omitted.image_items, 1);

    for _ in 0..MAX_OUTPUT_ITEMS {
        output.push(text(""));
    }
    let full = image("j".repeat(MAX_IMAGE_BYTES));
    output.push(full.clone());
    assert!(output.into_collected_output().items.last() == Some(&full));
}

#[test]
fn image_admission_reuses_space_freed_by_the_same_write() {
    let half = MAX_OUTPUT_ITEMS / 2;
    let mut output = collect((0..half).map(|_| text("")));
    output.push(image("i".repeat(MAX_IMAGE_BYTES)));
    for _ in 0..half - 1 {
        output.push(text(""));
    }
    let replacement = image("replacement".into());
    output.push(replacement.clone());
    let snapshot = output.into_collected_output();
    assert_eq!(snapshot.items.last(), Some(&replacement));
    assert_eq!(snapshot.gaps.len(), 1);
    assert_eq!(snapshot.gaps[0].omitted.image_items, 1);
}

#[test]
fn one_extra_text_byte_keeps_a_large_image_in_either_order() {
    let half = MAX_OUTPUT_BYTES / 2;
    let original = format!("{}b{}", "a".repeat(half), "c".repeat(half - 1));
    let retained = text(&[&original[..half], &original[half + 1..]].concat());
    let large_image = image("i".repeat(MAX_IMAGE_BYTES / 2 + 1));
    for image_index in [0, 1] {
        let mut items = vec![text(&original)];
        items.insert(image_index, large_image.clone());
        items.push(text("z"));
        let snapshot = collect(items).into_collected_output();
        let mut expected = vec![retained.clone(), text("z")];
        expected.insert(image_index, large_image.clone());
        assert!(snapshot.items == expected);
        let [gap] = snapshot.gaps.as_slice() else {
            panic!("expected one omitted byte")
        };
        assert_eq!(
            gap.boundary,
            OutputBoundary {
                item_index: 1 - image_index,
                text_offset: MAX_OUTPUT_BYTES / 2
            }
        );
        assert_eq!(gap.omitted.text_bytes, 1);
    }
}

#[test]
fn output_item_types_have_independent_byte_allowances() {
    let items = vec![
        text(&"t".repeat(MAX_OUTPUT_BYTES)),
        image("i".repeat(MAX_IMAGE_BYTES)),
        audio("a".into()),
    ];
    assert_eq!(collect(items.clone()).into_collected_output(), items.into());
}

#[test]
fn failed_delivery_keeps_the_observation_until_success() {
    // An intact image can move through a failed send and a retry without copying.
    let url = "i".repeat(MAX_IMAGE_BYTES);
    let ptr = url.as_ptr();
    let mut output = OutputCollector::new(CodeModeOutputTruncation::Server);
    output.push(image(url));
    assert!(!output.try_deliver(Err));
    assert!(output.try_deliver(|snapshot| {
        let snapshot = snapshot.into_collected_output();
        let [OutputItem::InputImage { image_url, .. }] = snapshot.items.as_slice() else {
            panic!("expected the original image")
        };
        assert_eq!(image_url.as_ptr(), ptr);
        Ok(())
    }));

    // The image allowance has reset. A failed send must also preserve a loss notice.
    let full = image("j".repeat(MAX_IMAGE_BYTES));
    output.push(full.clone());
    output.push(text(&"x".repeat(2 * MAX_OUTPUT_BYTES)));
    let mut expected = output.clone().into_collected_output();
    assert!(expected.items.first() == Some(&full));
    assert_eq!(expected.gaps.len(), 1);
    assert!(!output.try_deliver(Err));
    output.push(text(""));
    expected.items.push(text(""));
    assert!(output.try_deliver(|snapshot| {
        assert!(snapshot.into_collected_output() == expected);
        Ok(())
    }));
    assert_eq!(output.into_collected_output(), CollectedOutput::default());
}

#[test]
fn repeated_eviction_keeps_recent_writes_and_gaps_on_each_side_of_an_image() {
    let half = "p".repeat(MAX_OUTPUT_BYTES / 2);
    let image = image("i".repeat(2 * MAX_OUTPUT_BYTES));
    let mut output = collect([text(&half), text("b"), image.clone(), text(&half)]);
    let chunk = "x".repeat(64);
    let writes = 2 * MAX_OUTPUT_ITEMS;
    for _ in 0..writes {
        output.push(text(&chunk));
    }
    let snapshot = output.into_collected_output();
    let recent = MAX_OUTPUT_BYTES / 2 / chunk.len();
    assert_eq!(snapshot.items.len(), 2 + recent);
    assert!(snapshot.items.starts_with(&[text(&half), image]));
    assert!(
        snapshot.items[2..]
            .iter()
            .all(|item| matches!(item, OutputItem::InputText { text } if text == &chunk))
    );
    let [before, after] = snapshot.gaps.as_slice() else {
        panic!("expected gaps around the image")
    };
    assert_eq!(
        (before.boundary.item_index, after.boundary.item_index),
        (1, 2)
    );
    assert_eq!(before.omitted.text_bytes, 1);
    assert_eq!(after.omitted.text_bytes, (writes * chunk.len()) as u64);
    assert_eq!(after.omitted.text_items, (1 + writes - recent) as u64);
}

#[test]
fn terminal_error_matches_ordinary_text_across_loss_boundaries() {
    let label = "Script error:\n";
    // Rounding inward at the emoji can wrongly pull XYZ into the retained suffix.
    let error = format!(
        "{}XYZ{}💡{}",
        "p".repeat(MAX_OUTPUT_BYTES - label.len() - 3),
        "m".repeat(22),
        "s".repeat(MAX_OUTPUT_BYTES - 3),
    );
    let half = text(&"p".repeat(MAX_OUTPUT_BYTES / 2));
    for (prefix, gap_count) in [
        (Vec::new(), 1),
        (vec![text(""); MAX_OUTPUT_ITEMS / 2], 1),
        (vec![half.clone(), text("lost"), image("i".into()), half], 2),
    ] {
        let mut terminal = BoundedOutput::default();
        let mut ordinary = BoundedOutput::default();
        for item in prefix {
            terminal.push(item.clone());
            ordinary.push(item);
        }
        terminal.push_error(&error);
        ordinary.push(text(&format!("{label}{error}")));
        assert!(retained_capacity(&terminal) < MAX_OUTPUT_BYTES + TEXT_BLOCK_BYTES);
        let actual = terminal.into_collected_output();
        assert_eq!(actual.gaps.len(), gap_count);
        assert!(actual == ordinary.into_collected_output());
    }
}
