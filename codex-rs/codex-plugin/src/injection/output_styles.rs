//! Output style injection.

use crate::loader::PluginOutputStyle;
use std::path::PathBuf;

/// Injected output style for Codex systems.
#[derive(Debug, Clone)]
pub struct InjectedOutputStyle {
    /// Style name.
    pub name: String,
    /// Style description.
    pub description: String,
    /// Template content.
    pub template: String,
    /// Path to style file.
    pub path: PathBuf,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Convert a plugin output style to an injected output style.
pub fn convert_output_style(style: &PluginOutputStyle) -> Result<InjectedOutputStyle, String> {
    Ok(InjectedOutputStyle {
        name: style.name.clone(),
        description: style.description.clone(),
        template: style.template.clone(),
        path: style.path.clone(),
        source_plugin: style.source_plugin.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_output_style() {
        let style = PluginOutputStyle {
            name: "compact".to_string(),
            description: "Compact output".to_string(),
            template: "{{ content }}".to_string(),
            path: PathBuf::from("/test/compact.md"),
            source_plugin: "test-plugin".to_string(),
        };

        let converted = convert_output_style(&style).unwrap();
        assert_eq!(converted.name, "compact");
        assert_eq!(converted.template, "{{ content }}");
    }
}
