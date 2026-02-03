use crate::artificial_messages::ArtificialMessageParseError;

macro_rules! render_field {
    ($tagged:ident, $raw:ident, tagged, $value:ident, $field_tag:literal) => {{
        $tagged.push_str(&$crate::artificial_messages::macros::xml(
            $field_tag, $value,
        ));
        $tagged.push('\n');
    }};
    ($tagged:ident, $raw:ident, raw, $value:ident) => {{
        $raw = Some($value.as_str());
    }};
}

macro_rules! render_fields {
    ($($kind:ident($value:ident $(, $field_tag:literal)?)),* $(,)?) => {{
        #[allow(unused_mut, unused_assignments)]
        let mut tagged = String::new();
        #[allow(unused_mut, unused_assignments)]
        let mut raw = None;
        $(
            render_field!(tagged, raw, $kind, $value $(, $field_tag)?);
        )*

        if tagged.is_empty() {
            raw.unwrap_or_default().to_string()
        } else {
            let mut out = String::new();
            out.push('\n');
            out.push_str(&tagged);
            if let Some(raw) = raw {
                out.push_str(raw);
                out.push('\n');
            }
            out
        }
    }};
}

macro_rules! parse_field {
    ($remaining:ident, $consumed_tagged:ident, $msg_tag:expr, tagged, $field:ident, $field_tag:literal) => {
        let tagged_input = $remaining.strip_prefix('\n').unwrap_or($remaining);
        let ($field, rest) =
            $crate::artificial_messages::macros::extract_prefixed_tag(tagged_input, $field_tag)
                .ok_or_else(|| ArtificialMessageParseError::MissingField {
                    message_tag: $msg_tag,
                    field_tag: $field_tag,
                })?;
        let $field = $field.to_string();
        $remaining = rest;
        if !$consumed_tagged {
            $consumed_tagged = true;
        }
    };
    ($remaining:ident, $consumed_tagged:ident, $msg_tag:expr, raw, $field:ident) => {
        let mut value = $remaining;
        if $consumed_tagged {
            value = value.strip_prefix('\n').unwrap_or(value);
            value = value.strip_suffix('\n').unwrap_or(value);
        }
        let $field = value.to_string();
        $remaining = "";
    };
}

macro_rules! parse_variant {
    ($variant:ident, $inner:expr, $msg_tag:expr, $($kind:ident($field:ident $(, $field_tag:literal)?)),* $(,)?) => {{
        #[allow(unused_mut, unused_assignments)]
        let mut remaining = $inner;
        #[allow(unused_mut, unused_assignments)]
        let mut consumed_tagged = false;
        $(
            parse_field!(
                remaining,
                consumed_tagged,
                $msg_tag,
                $kind,
                $field
                $(, $field_tag)?
            );
        )*
        let _ = &remaining;
        let _ = consumed_tagged;
        Ok(ArtificialMessage::$variant { $( $field ),* })
    }};
}

macro_rules! artificial_messages {
    (
        $(
            $variant:ident {
                tag: $tag:path,
                role: $role:literal,
                fields: { $($kind:ident($field:ident $(, $field_tag:literal)?)),* $(,)? }
            }
        ),* $(,)?
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ArtificialMessage {
            $(
                $variant { $( $field: String ),* }
            ),*
        }

        impl ArtificialMessage {
            pub fn tag(&self) -> &'static str {
                match self {
                    $( Self::$variant { .. } => $tag ),*
                }
            }

            pub fn role(&self) -> &'static str {
                match self {
                    $( Self::$variant { .. } => $role ),*
                }
            }

            pub fn render(&self) -> String {
                match self {
                    $(
                        Self::$variant { $( $field ),* } => {
                            let payload = render_fields!($($kind($field $(, $field_tag)?)),*);
                            $crate::artificial_messages::macros::xml($tag, payload)
                        }
                    ),*
                }
            }

            pub fn parse(input: &str) -> Result<Self, ArtificialMessageParseError> {
                let (tag, inner) = $crate::artificial_messages::macros::split_outer_tag(input)?;
                match tag {
                    $(
                        $tag => parse_variant!($variant, inner, $tag, $($kind($field $(, $field_tag)?)),*),
                    )*
                    unknown => Err(ArtificialMessageParseError::UnknownTopLevelTag(
                        unknown.to_string(),
                    )),
                }
            }

            pub fn detect_tag(input: &str) -> Option<&'static str> {
                let (tag, _) = $crate::artificial_messages::macros::split_outer_tag(input).ok()?;
                match tag {
                    $( $tag => Some($tag), )*
                    _ => None,
                }
            }

            pub fn is_artificial(input: &str) -> bool {
                Self::detect_tag(input).is_some()
            }

            pub fn to_response_item(&self) -> ResponseItem {
                ResponseItem::Message {
                    role: self.role().to_string(),
                    id: None,
                    content: vec![ContentItem::InputText {
                        text: self.render(),
                    }],
                    end_turn: None,
                    phase: None,
                }
            }
        }
    };
}

pub(crate) fn split_outer_tag(input: &str) -> Result<(&str, &str), ArtificialMessageParseError> {
    let text = input.trim();
    if !text.starts_with('<') {
        return Err(ArtificialMessageParseError::InvalidEnvelope);
    }

    let Some(open_end) = text.find('>') else {
        return Err(ArtificialMessageParseError::InvalidEnvelope);
    };

    let tag = &text[1..open_end];
    if tag.is_empty() || tag.starts_with('/') || tag.chars().any(char::is_whitespace) {
        return Err(ArtificialMessageParseError::InvalidEnvelope);
    }

    let close = format!("</{tag}>");
    let Some(inner) = text[open_end + 1..].strip_suffix(&close) else {
        return Err(ArtificialMessageParseError::InvalidEnvelope);
    };

    Ok((tag, inner))
}

pub(crate) fn extract_prefixed_tag<'a>(
    text: &'a str,
    field_tag: &'static str,
) -> Option<(&'a str, &'a str)> {
    let open = format!("<{field_tag}>");
    let close = format!("</{field_tag}>");
    let after_open = text.strip_prefix(&open)?;
    let end = after_open.find(&close)?;
    let value = &after_open[..end];
    let rest = &after_open[end + close.len()..];
    Some((value, rest))
}

pub(crate) fn xml(tag: &str, payload: impl std::fmt::Display) -> String {
    format!("<{tag}>{payload}</{tag}>")
}
