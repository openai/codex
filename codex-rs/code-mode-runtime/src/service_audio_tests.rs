//! Covers per-item short-WAV omissions without disrupting other Code Mode outputs.

use super::cell_id;
use super::execute;
use super::execute_request;
use crate::ExecuteRequest;
use crate::FunctionCallOutputContentItem;
use crate::InProcessCodeModeSession;
use crate::RuntimeResponse;
use crate::WaitOutcome;
use crate::WaitRequest;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use pretty_assertions::assert_eq;

const SHORT_AUDIO_OMISSION_TEXT: &str =
    "Audio output omitted because the clip is shorter than 25 ms; use a longer clip.";

fn pcm16_wav(sample_rate: u32, frames: usize) -> Vec<u8> {
    let data_size = u32::try_from(frames * 2).unwrap();
    let mut wav = b"RIFF".to_vec();
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.resize(44 + frames * 2, /*value*/ 0);
    wav
}

#[tokio::test]
async fn audio_helper_omits_short_wav_in_all_input_forms() {
    for (sample_rate, frames, omitted) in [
        (24_000, 0, true),
        (24_000, 1, true),
        (24_000, 599, true),
        (24_000, 600, false),
        (24_000, 601, false),
        (44_100, 223, true),
        (44_100, 1_102, true),
        (44_100, 1_103, false),
        (30_870, 223, true),
        (30_870, 771, true),
        (30_870, 772, false),
        (34_398, 221, true),
        (34_398, 859, true),
        (34_398, 860, false),
    ] {
        let payload = BASE64_STANDARD.encode(pcm16_wav(sample_rate, frames));
        let audio_url = format!("data:audio/wav;base64,{payload}");
        let service = InProcessCodeModeSession::new();
        let response = execute(
            &service,
            ExecuteRequest {
                source: format!(
                    r#"audio({audio_url:?}); audio({{audio_url:{audio_url:?}}});
audio({{type:"audio",mimeType:"audio/wav",data:{payload:?}}});"#
                ),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;
        let expected = if omitted {
            FunctionCallOutputContentItem::InputText {
                text: SHORT_AUDIO_OMISSION_TEXT.to_string(),
            }
        } else {
            FunctionCallOutputContentItem::InputAudio { audio_url }
        };
        assert_eq!(
            response,
            RuntimeResponse::Result {
                code_mode_host_duration: None,
                cell_id: cell_id("1"),
                content_items: vec![expected; 3],
                error_text: None,
            },
            "sample rate {sample_rate}, frames {frames}"
        );
    }
}

#[tokio::test]
async fn audio_helper_bounds_wav_data_and_preserves_outputs_after_yield() {
    let mut metadata_short = pcm16_wav(/*sample_rate*/ 24_000, /*frames*/ 1);
    let mut valid = pcm16_wav(/*sample_rate*/ 24_000, /*frames*/ 600);
    let mut list = b"LIST".to_vec();
    list.extend_from_slice(&(4_u32 + 8 + 2048).to_le_bytes());
    list.extend_from_slice(b"INFOINAM");
    list.extend_from_slice(&2048_u32.to_le_bytes());
    list.resize(8 + 4 + 8 + 2048, /*value*/ 0);
    list.extend_from_slice(b"JUNK\x01\0\0\0x\0");
    for wav in [&mut metadata_short, &mut valid] {
        drop(wav.splice(36..36, list.iter().copied()));
        let riff_size = u32::try_from(wav.len() - 8).unwrap();
        wav[4..8].copy_from_slice(&riff_size.to_le_bytes());
    }
    let mut truncated = pcm16_wav(/*sample_rate*/ 24_000, /*frames*/ 0);
    truncated[4..8].copy_from_slice(&48_036_u32.to_le_bytes());
    truncated[40..44].copy_from_slice(&48_000_u32.to_le_bytes());
    let mut bounded = pcm16_wav(/*sample_rate*/ 24_000, /*frames*/ 600);
    bounded[40..44].copy_from_slice(&2_u32.to_le_bytes());
    let urls = [metadata_short, truncated, bounded, valid]
        .map(|wav| format!("data:audio/wav;base64,{}", BASE64_STANDARD.encode(wav)));
    let audio_urls = serde_json::to_string(&urls).unwrap();
    let service = InProcessCodeModeSession::new();
    let response = execute(
        &service,
        ExecuteRequest {
            source: format!(
                "text('before'); await yield_control(); \
                 for (const audio_url of {audio_urls}) audio({{audio_url}}); text('after');"
            ),
            yield_time_ms: None,
            ..execute_request("")
        },
    )
    .await;
    assert_eq!(
        response,
        RuntimeResponse::Yielded {
            code_mode_host_duration: None,
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
        }
    );
    let mut expected = vec![
        FunctionCallOutputContentItem::InputText {
            text: SHORT_AUDIO_OMISSION_TEXT.to_string(),
        };
        3
    ];
    expected.push(FunctionCallOutputContentItem::InputAudio {
        audio_url: urls[3].clone(),
    });
    expected.push(FunctionCallOutputContentItem::InputText {
        text: "after".to_string(),
    });
    assert_eq!(
        service
            .wait(
                WaitRequest {
                    cell_id: cell_id("1"),
                    yield_time_ms: 60_000,
                },
                /*preempt*/ None,
            )
            .await
            .unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Result {
            code_mode_host_duration: None,
            cell_id: cell_id("1"),
            content_items: expected,
            error_text: None,
        })
    );
}
