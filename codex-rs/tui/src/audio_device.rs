use codex_core::config::Config;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use tracing::warn;

use crate::app_event::VoiceAudioDeviceKind;

const PREFERRED_INPUT_SAMPLE_RATE: u32 = 24_000;
const PREFERRED_INPUT_CHANNELS: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VoiceAudioDevice {
    pub(crate) id: String,
    pub(crate) label: String,
}

pub(crate) fn list_voice_audio_devices(
    kind: VoiceAudioDeviceKind,
) -> Result<Vec<VoiceAudioDevice>, String> {
    let host = cpal::default_host();
    Ok(devices(&host, kind)?
        .into_iter()
        .filter_map(|device| {
            let label = device.description().ok()?.to_string();
            let id = device.id().ok()?.to_string();
            Some(VoiceAudioDevice { id, label })
        })
        .collect())
}

pub(crate) fn select_configured_input_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(
        VoiceAudioDeviceKind::Microphone,
        config.realtime_audio.microphone.as_deref(),
    )
}

pub(crate) fn select_configured_output_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(
        VoiceAudioDeviceKind::Speaker,
        config.realtime_audio.speaker.as_deref(),
    )
}

pub(crate) fn select_input_device_and_config(
    device_id: Option<&str>,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(VoiceAudioDeviceKind::Microphone, device_id)
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
    kind: VoiceAudioDeviceKind,
    configured_device_id: Option<&str>,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let host = cpal::default_host();
    let selected = configured_device_id
        .and_then(|device_id| configured_device(&host, kind, device_id))
        .or_else(|| {
            let default_device = default_device(&host, kind);
            if let Some(device_id) = configured_device_id && default_device.is_some() {
                warn!(
                    "configured {} audio device `{device_id}` was unavailable; falling back to system default",
                    kind.noun()
                );
            }
            default_device
        })
        .ok_or_else(|| missing_device_error(kind, configured_device_id))?;

    let stream_config = match kind {
        VoiceAudioDeviceKind::Microphone => preferred_input_config(&selected)?,
        VoiceAudioDeviceKind::Speaker => default_config(&selected, kind)?,
    };
    Ok((selected, stream_config))
}

fn configured_device(
    host: &cpal::Host,
    kind: VoiceAudioDeviceKind,
    device_id: &str,
) -> Option<cpal::Device> {
    let parsed_id = device_id.parse().ok()?;
    let device = host.device_by_id(&parsed_id)?;

    match kind {
        VoiceAudioDeviceKind::Microphone => device.default_input_config().ok().map(|_| device),
        VoiceAudioDeviceKind::Speaker => device.default_output_config().ok().map(|_| device),
    }
}

fn devices(host: &cpal::Host, kind: VoiceAudioDeviceKind) -> Result<Vec<cpal::Device>, String> {
    match kind {
        VoiceAudioDeviceKind::Microphone => host
            .input_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate input audio devices: {err}")),
        VoiceAudioDeviceKind::Speaker => host
            .output_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate output audio devices: {err}")),
    }
}

fn default_device(host: &cpal::Host, kind: VoiceAudioDeviceKind) -> Option<cpal::Device> {
    match kind {
        VoiceAudioDeviceKind::Microphone => host.default_input_device(),
        VoiceAudioDeviceKind::Speaker => host.default_output_device(),
    }
}

fn default_config(
    device: &cpal::Device,
    kind: VoiceAudioDeviceKind,
) -> Result<cpal::SupportedStreamConfig, String> {
    match kind {
        VoiceAudioDeviceKind::Microphone => device
            .default_input_config()
            .map_err(|err| format!("failed to get default input config: {err}")),
        VoiceAudioDeviceKind::Speaker => device
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

fn missing_device_error(kind: VoiceAudioDeviceKind, configured_device_id: Option<&str>) -> String {
    match (kind, configured_device_id) {
        (VoiceAudioDeviceKind::Microphone, Some(device_id)) => {
            format!(
                "configured microphone `{device_id}` was unavailable and no default input audio device was found"
            )
        }
        (VoiceAudioDeviceKind::Speaker, Some(device_id)) => {
            format!(
                "configured speaker `{device_id}` was unavailable and no default output audio device was found"
            )
        }
        (VoiceAudioDeviceKind::Microphone, None) => "no input audio device available".to_string(),
        (VoiceAudioDeviceKind::Speaker, None) => "no output audio device available".to_string(),
    }
}
