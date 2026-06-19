use std::borrow::Cow;

use serde_yaml::Mapping;
use serde_yaml::Value;

pub(crate) const MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS: usize = 1_024;
pub(crate) const TRUNCATED_SKILL_DESCRIPTION_SUFFIX: &str = "...";

pub(crate) fn truncate_skill_description(description: &str) -> Cow<'_, str> {
    if description
        .char_indices()
        .nth(MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS)
        .is_none()
    {
        return Cow::Borrowed(description);
    }

    let prefix_chars = MAX_MODEL_VISIBLE_SKILL_DESCRIPTION_CHARS
        .saturating_sub(TRUNCATED_SKILL_DESCRIPTION_SUFFIX.chars().count());
    let prefix_end = description
        .char_indices()
        .nth(prefix_chars)
        .map_or(description.len(), |(index, _)| index);
    let mut truncated = description[..prefix_end].to_string();
    truncated.push_str(TRUNCATED_SKILL_DESCRIPTION_SUFFIX);
    Cow::Owned(truncated)
}

pub(crate) fn render_model_visible_skill_contents(contents: &str) -> Cow<'_, str> {
    let Some(rest) = contents
        .strip_prefix("---\n")
        .or_else(|| contents.strip_prefix("---\r\n"))
    else {
        return Cow::Borrowed(contents);
    };
    let Some((frontmatter_end, body_start)) = [
        "\r\n---\r\n",
        "\r\n---\n",
        "\n---\r\n",
        "\n---\n",
        "\r\n---",
        "\n---",
    ]
    .into_iter()
    .filter_map(|delimiter| rest.find(delimiter).map(|end| (end, end + delimiter.len())))
    .min_by_key(|(end, _body_start)| *end) else {
        return Cow::Borrowed(contents);
    };

    let raw_frontmatter = &rest[..frontmatter_end];
    let Ok(mut frontmatter) = serde_yaml::from_str::<Mapping>(raw_frontmatter) else {
        return Cow::Borrowed(contents);
    };
    let mut changed = false;
    let description_key = Value::String("description".to_string());
    if let Some(Value::String(description)) = frontmatter.get_mut(&description_key)
        && let Cow::Owned(truncated) = truncate_skill_description(description)
    {
        *description = truncated;
        changed = true;
    }
    let metadata_key = Value::String("metadata".to_string());
    let short_description_key = Value::String("short-description".to_string());
    if let Some(Value::Mapping(metadata)) = frontmatter.get_mut(&metadata_key)
        && let Some(Value::String(short_description)) = metadata.get_mut(&short_description_key)
        && let Cow::Owned(truncated) = truncate_skill_description(short_description)
    {
        *short_description = truncated;
        changed = true;
    }
    if !changed {
        return Cow::Borrowed(contents);
    }

    let Ok(frontmatter) = serde_yaml::to_string(&frontmatter) else {
        return Cow::Borrowed(contents);
    };
    let body = &rest[body_start..];
    let mut rendered = String::with_capacity(contents.len());
    rendered.push_str("---\n");
    rendered.push_str(frontmatter.trim_end());
    rendered.push_str("\n---\n");
    rendered.push_str(body);
    Cow::Owned(rendered)
}

#[cfg(test)]
#[path = "model_visible_tests.rs"]
mod tests;
