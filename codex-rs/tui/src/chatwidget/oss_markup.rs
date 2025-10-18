#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OssSegment {
    Analysis(String),
    Final(String),
    ToolRequest {
        tool: String,
        payload: String,
    },
    ToolOutput {
        payload: String,
    },
    Other {
        channels: Vec<String>,
        payload: String,
    },
}

pub(crate) fn parse_oss_segments(message: &str) -> Option<Vec<OssSegment>> {
    let mut segments: Vec<OssSegment> = Vec::new();
    let mut active_channels: Vec<String> = Vec::new();
    let mut pending_channels: Vec<String> = Vec::new();
    let mut pending_message: Option<String> = None;

    for token in message.split("<|").filter(|token| !token.is_empty()) {
        if let Some(rest) = token.strip_prefix("start|>") {
            if pending_message.is_some() {
                pending_channels.clear();
                pending_message = None;
            }
            active_channels.clear();
            if rest.trim().is_empty() {
                continue;
            }
            continue;
        }
        if let Some(rest) = token.strip_prefix("channel|>") {
            active_channels.push(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = token.strip_prefix("message|>") {
            pending_message = Some(rest.to_string());
            pending_channels = active_channels.clone();
            continue;
        }
        if token.starts_with("end|>") {
            if let Some(msg) = pending_message.take() {
                if let Some(segment) = classify_segment(pending_channels.clone(), msg, false) {
                    segments.push(segment);
                }
            }
            pending_channels.clear();
            active_channels.clear();
            continue;
        }
        if token.starts_with("call|>") {
            if let Some(msg) = pending_message.take() {
                if let Some(segment) = classify_segment(pending_channels.clone(), msg, true) {
                    segments.push(segment);
                }
            }
            pending_channels.clear();
            active_channels.clear();
            continue;
        }
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

fn classify_segment(channels: Vec<String>, payload: String, is_call: bool) -> Option<OssSegment> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        // Keep tool output segments even if whitespace-only payload is unlikely.
        if is_call
            && channels
                .iter()
                .any(|ch| ch.starts_with("commentary") || ch.starts_with("observation"))
        {
            return Some(OssSegment::ToolOutput { payload });
        }
        return None;
    }

    if is_call {
        if let Some(tool_channel) = channels.iter().find(|ch| ch.starts_with("commentary to=")) {
            let tool = tool_channel
                .trim_start_matches("commentary to=")
                .trim()
                .to_string();
            if !tool.is_empty() {
                return Some(OssSegment::ToolRequest { tool, payload });
            }
        }

        if channels.iter().any(|ch| ch.starts_with("commentary")) {
            return Some(OssSegment::ToolOutput { payload });
        }
    }

    if channels.iter().any(|ch| ch.starts_with("analysis")) {
        return Some(OssSegment::Analysis(payload));
    }

    if channels.iter().any(|ch| ch.starts_with("final")) {
        return Some(OssSegment::Final(payload));
    }

    Some(OssSegment::Other { channels, payload })
}

pub(crate) fn strip_markup(message: &str) -> String {
    let mut parts = message.split("<|");
    let mut cleaned = String::new();
    if let Some(first) = parts.next() {
        cleaned.push_str(first);
    }

    for token in parts {
        if let Some(rest) = token.strip_prefix("message|>") {
            cleaned.push_str(rest);
            continue;
        }
        if token.starts_with("channel|>")
            || token.starts_with("start|>")
            || token.starts_with("end|>")
            || token.starts_with("call|>")
        {
            continue;
        }
        if let Some(pos) = token.find("|>") {
            cleaned.push_str(&token[pos + 2..]);
        } else {
            cleaned.push_str(token);
        }
    }

    cleaned
}

#[cfg(test)]
mod tests {
    use super::OssSegment;
    use super::parse_oss_segments;
    use super::strip_markup;

    #[test]
    fn parse_sample_sequence() {
        let sample = "<|channel|>analysis<|message|>Reasoning text.<|end|><|start|>assistant<|channel|>commentary to=functions.shell<|channel|>analysis<|message|>{\"command\":[\"bash\",\"-lc\",\"ls\"]}<|call|><|start|>assistant<|channel|>commentary<|message|>{\"output\":\"ok\",\"metadata\":{\"exit_code\":0,\"duration_seconds\":0.1}}<|call|><|start|>assistant<|channel|>final<|message|>Final text.<|end|>";

        let segments = parse_oss_segments(sample).expect("segments");
        assert_eq!(
            segments,
            vec![
                OssSegment::Analysis("Reasoning text.".to_string()),
                OssSegment::ToolRequest {
                    tool: "functions.shell".to_string(),
                    payload: "{\"command\":[\"bash\",\"-lc\",\"ls\"]}".to_string(),
                },
                OssSegment::ToolOutput {
                    payload:
                        "{\"output\":\"ok\",\"metadata\":{\"exit_code\":0,\"duration_seconds\":0.1}}"
                            .to_string(),
                },
                OssSegment::Final("Final text.".to_string()),
            ]
        );
    }

    #[test]
    fn strip_markup_removes_tokens() {
        let sample = "<|channel|>analysis<|message|>Reasoning.<|end|><|start|>assistant<|channel|>final<|message|>Answer.<|end|>";
        assert_eq!(strip_markup(sample), "Reasoning.Answer.");
    }
}
