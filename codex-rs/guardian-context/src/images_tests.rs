use super::MAX_TRANSCRIPT_IMAGE_BYTES;
use super::TranscriptImageInput;
use super::TranscriptImages;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

fn image(url: &str) -> ContentItem {
    ContentItem::InputImage {
        image: ImageReference::Inline {
            image_url: url.into(),
        },
        detail: Some(ImageDetail::High),
    }
}

#[test]
fn image_selection_preserves_source_policy_order_and_both_limits() {
    let history = [
        ResponseItem::Message {
            id: None,
            role: "user".into(),
            content: ["first", "second", "third", "fourth"].map(image).to_vec(),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("screenshot".into()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "tool".into(),
                    },
                    detail: Some(ImageDetail::High),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let repl = [image("repl")];
    let input = TranscriptImageInput {
        enabled: true,
        include_tool_outputs: true,
        node_repl_images: &repl,
    };
    assert_eq!(
        TranscriptImages::collect(&history, input),
        TranscriptImages {
            images: ["third", "fourth", "tool", "repl"].map(image).to_vec(),
            omitted_bytes: "firstsecond".len(),
        }
    );
    assert_eq!(
        TranscriptImages::collect(
            &history,
            TranscriptImageInput {
                include_tool_outputs: false,
                ..input
            }
        ),
        TranscriptImages {
            images: ["first", "second", "third", "fourth"].map(image).to_vec(),
            omitted_bytes: 0,
        }
    );
    assert_eq!(
        TranscriptImages::collect(
            &history,
            TranscriptImageInput {
                enabled: false,
                ..input
            }
        ),
        TranscriptImages::default()
    );

    let oversized = "x".repeat(MAX_TRANSCRIPT_IMAGE_BYTES + 1);
    let fits_alone = "y".repeat(MAX_TRANSCRIPT_IMAGE_BYTES);
    let repl = [image(&oversized), image(&fits_alone), image("recent")];
    assert_eq!(
        TranscriptImages::collect(
            &[],
            TranscriptImageInput {
                node_repl_images: &repl,
                ..input
            }
        ),
        TranscriptImages {
            images: vec![image("recent")],
            omitted_bytes: oversized.len() + fits_alone.len(),
        }
    );
}
