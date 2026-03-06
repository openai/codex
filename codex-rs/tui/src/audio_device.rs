use codex_core::config::Config;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use tracing::warn;

use crate::app_event::RealtimeAudioDeviceKind;

const PREFERRED_INPUT_SAMPLE_RATE: u32 = 24_000;
const PREFERRED_INPUT_CHANNELS: u16 = 1;

pub(crate) fn list_realtime_audio_device_names(
    kind: RealtimeAudioDeviceKind,
) -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let mut device_names = Vec::new();
    for device in devices(&host, kind)? {
        let Ok(name) = device
            .description()
            .map(|description| description.to_string())
        else {
            continue;
        };
        if !device_names.contains(&name) {
            device_names.push(name);
        }
    }
    Ok(device_names)
}

pub(crate) fn select_configured_input_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(RealtimeAudioDeviceKind::Microphone, config)
}

pub(crate) fn select_configured_output_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(RealtimeAudioDeviceKind::Speaker, config)
}

pub(crate) fn preferred_input_config(
    device: &cpal::Device,
) -> Result<cpal::SupportedStreamConfig, String> {
    let supported_configs = device
        .supported_input_configs()
        .map_err(|err| format!("failed to enumerate input audio configs: {err}"))?;

    supported_configs
        .filter_map(|range| {
            let sample_format_rank = match range.sample_format() {
                cpal::SampleFormat::I16 => 0u8,
                cpal::SampleFormat::U16 => 1u8,
                cpal::SampleFormat::F32 => 2u8,
                _ => return None,
            };
            let sample_rate = preferred_input_sample_rate(&range);
            let sample_rate_penalty = sample_rate.abs_diff(PREFERRED_INPUT_SAMPLE_RATE);
            let channel_penalty = range.channels().abs_diff(PREFERRED_INPUT_CHANNELS);
            Some((
                (sample_rate_penalty, channel_penalty, sample_format_rank),
                range.with_sample_rate(sample_rate),
            ))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, config)| config)
        .or_else(|| device.default_input_config().ok())
        .ok_or_else(|| "failed to get default input config".to_string())
}

fn select_device_and_config(
    kind: RealtimeAudioDeviceKind,
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let host = cpal::default_host();
    let configured_label = configured_label(kind, config);
    let selected = configured_label
        .and_then(|label| find_device_by_label(&host, kind, label))
        .or_else(|| {
            let default_device = default_device(&host, kind);
            if let Some(label) = configured_label && default_device.is_some() {
                warn!(
                    "configured {} audio device `{label}` was unavailable; falling back to system default",
                    kind.noun()
                );
            }
            default_device
        })
        .ok_or_else(|| missing_device_error(kind, configured_label))?;

    let stream_config = match kind {
        RealtimeAudioDeviceKind::Microphone => preferred_input_config(&selected)?,
        RealtimeAudioDeviceKind::Speaker => default_config(&selected, kind)?,
    };
    Ok((selected, stream_config))
}

fn configured_label(kind: RealtimeAudioDeviceKind, config: &Config) -> Option<&str> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => config.realtime_audio.microphone.as_deref(),
        RealtimeAudioDeviceKind::Speaker => config.realtime_audio.speaker.as_deref(),
    }
}

fn find_device_by_label(
    host: &cpal::Host,
    kind: RealtimeAudioDeviceKind,
    label: &str,
) -> Option<cpal::Device> {
    let devices = devices(host, kind).ok()?;
    devices.into_iter().find(|device| {
        device
            .description()
            .ok()
            .map(|description| description.to_string())
            .as_deref()
            == Some(label)
    })
}

fn devices(host: &cpal::Host, kind: RealtimeAudioDeviceKind) -> Result<Vec<cpal::Device>, String> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => host
            .input_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate input audio devices: {err}")),
        RealtimeAudioDeviceKind::Speaker => host
            .output_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate output audio devices: {err}")),
    }
}

fn default_device(host: &cpal::Host, kind: RealtimeAudioDeviceKind) -> Option<cpal::Device> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => host.default_input_device(),
        RealtimeAudioDeviceKind::Speaker => host.default_output_device(),
    }
}

fn default_config(
    device: &cpal::Device,
    kind: RealtimeAudioDeviceKind,
) -> Result<cpal::SupportedStreamConfig, String> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => device
            .default_input_config()
            .map_err(|err| format!("failed to get default input config: {err}")),
        RealtimeAudioDeviceKind::Speaker => device
            .default_output_config()
            .map_err(|err| format!("failed to get default output config: {err}")),
    }
}

fn preferred_input_sample_rate(range: &cpal::SupportedStreamConfigRange) -> cpal::SampleRate {
    let min = range.min_sample_rate();
    let max = range.max_sample_rate();
    if (min..=max).contains(&PREFERRED_INPUT_SAMPLE_RATE) {
        PREFERRED_INPUT_SAMPLE_RATE
    } else if PREFERRED_INPUT_SAMPLE_RATE < min {
        min
    } else {
        max
    }
}

fn missing_device_error(kind: RealtimeAudioDeviceKind, configured_label: Option<&str>) -> String {
    match (kind, configured_label) {
        (RealtimeAudioDeviceKind::Microphone, Some(label)) => {
            format!(
                "configured microphone `{label}` was unavailable and no default input audio device was found"
            )
        }
        (RealtimeAudioDeviceKind::Speaker, Some(label)) => {
            format!(
                "configured speaker `{label}` was unavailable and no default output audio device was found"
            )
        }
        (RealtimeAudioDeviceKind::Microphone, None) => {
            "no input audio device available".to_string()
        }
        (RealtimeAudioDeviceKind::Speaker, None) => "no output audio device available".to_string(),
    }
}
