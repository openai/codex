use std::str;

pub(crate) struct Cache<T> {
    attempted: bool,
    value: Option<T>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self {
            attempted: false,
            value: None,
        }
    }
}

impl<T: Copy> Cache<T> {
    pub(crate) fn get_or_init_with(&mut self, mut init: impl FnMut() -> Option<T>) -> Option<T> {
        if !self.attempted {
            self.value = init();
            self.attempted = true;
        }
        self.value
    }

    pub(crate) fn refresh_with(&mut self, mut init: impl FnMut() -> Option<T>) -> Option<T> {
        self.value = init();
        self.attempted = true;
        self.value
    }
}

pub(crate) fn apply_palette_responses(
    buffer: &mut Vec<u8>,
    palette: &mut [Option<(u8, u8, u8)>; 256],
) -> usize {
    let mut newly_filled = 0;

    while let Some(start) = buffer.windows(2).position(|window| window == [0x1b, b']']) {
        if start > 0 {
            buffer.drain(..start);
            continue;
        }

        let mut index = 2; // skip ESC ]
        let mut terminator_len = None;
        while index < buffer.len() {
            match buffer[index] {
                0x07 => {
                    terminator_len = Some(1);
                    break;
                }
                0x1b if index + 1 < buffer.len() && buffer[index + 1] == b'\\' => {
                    terminator_len = Some(2);
                    break;
                }
                _ => index += 1,
            }
        }

        let Some(terminator_len) = terminator_len else {
            break;
        };

        let end = index;
        let parsed = str::from_utf8(&buffer[2..end])
            .ok()
            .and_then(parse_palette_message);
        let processed = end + terminator_len;
        buffer.drain(..processed);

        if let Some((slot, color)) = parsed
            && palette[slot].is_none()
        {
            palette[slot] = Some(color);
            newly_filled += 1;
        }
    }

    newly_filled
}

pub(crate) fn parse_osc_color(buffer: &[u8], code: u8) -> Option<(u8, u8, u8)> {
    let text = str::from_utf8(buffer).ok()?;
    let prefix = match code {
        10 => "\u{1b}]10;",
        11 => "\u{1b}]11;",
        _ => return None,
    };
    let start = text.rfind(prefix)?;
    let after_prefix = &text[start + prefix.len()..];
    let end_bel = after_prefix.find('\u{7}');
    let end_st = after_prefix.find("\u{1b}\\");
    let end_idx = match (end_bel, end_st) {
        (Some(bel), Some(st)) => bel.min(st),
        (Some(bel), None) => bel,
        (None, Some(st)) => st,
        (None, None) => return None,
    };
    let payload = after_prefix[..end_idx].trim();
    parse_color_payload(payload)
}

fn parse_palette_message(message: &str) -> Option<(usize, (u8, u8, u8))> {
    let mut parts = message.splitn(3, ';');
    if parts.next()? != "4" {
        return None;
    }
    let index: usize = parts.next()?.trim().parse().ok()?;
    if index >= 256 {
        return None;
    }
    let payload = parts.next()?;
    let (model, values) = payload.split_once(':')?;
    if model != "rgb" && model != "rgba" {
        return None;
    }
    let mut components = values.split('/');
    let r = parse_component(components.next()?)?;
    let g = parse_component(components.next()?)?;
    let b = parse_component(components.next()?)?;
    Some((index, (r, g, b)))
}

fn parse_color_payload(payload: &str) -> Option<(u8, u8, u8)> {
    if payload.is_empty() || payload == "?" {
        return None;
    }
    let (model, values) = payload.split_once(':')?;
    if model != "rgb" && model != "rgba" {
        return None;
    }
    let mut parts = values.split('/');
    let r = parse_component(parts.next()?)?;
    let g = parse_component(parts.next()?)?;
    let b = parse_component(parts.next()?)?;
    Some((r, g, b))
}

fn parse_component(component: &str) -> Option<u8> {
    let trimmed = component.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bits = trimmed.len().checked_mul(4)?;
    if bits == 0 || bits > 64 {
        return None;
    }
    let max = if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    let value = u64::from_str_radix(trimmed, 16).ok()?;
    Some(((value * 255 + max / 2) / max) as u8)
}
