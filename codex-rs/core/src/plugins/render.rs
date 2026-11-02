#[cfg(test)]
use crate::context::AvailablePluginsInstructions;
#[cfg(test)]
use crate::context::ContextualUserFragment;
use crate::plugins::PluginCapabilitySummary;
use codex_features::Feature;
use codex_features::Features;
use codex_prompts::ResolvedSelectedPluginMessages;
#[cfg(test)]
use codex_protocol_test_support::ContentItemTestExt;
use codex_utils_string::take_bytes_at_char_boundary;

const MAX_EXPLICIT_PLUGIN_INSTRUCTIONS_BYTES: usize = 4 * 1024;

#[cfg(test)]
pub(crate) fn render_plugins_section(plugins: &[PluginCapabilitySummary]) -> Option<String> {
    (!plugins.is_empty()).then(|| {
        AvailablePluginsInstructions::default()
            .render()
            .expect_text()
    })
}

pub(crate) fn render_explicit_plugin_instructions(
    plugin: &PluginCapabilitySummary,
    available_mcp_servers: &[String],
    available_apps: &[String],
    features: &Features,
    messages: &ResolvedSelectedPluginMessages,
) -> Option<String> {
    if features.enabled(Feature::SimplifiedSelectionHints) {
        return (plugin.has_skills || !available_apps.is_empty() || !available_mcp_servers.is_empty())
            .then(|| {
                bound_explicit_plugin_instructions(
                    format!(
                        "The user selected the `{}` plugin. Use its available skills and tools to complete this request.",
                        plugin.display_name
                    ),
                    "\n[Plugin selection truncated to fit the context limit.]",
                )
            });
    }

    let mut lines = vec![
        messages
            .intro
            .replace("{{ plugin_name }}", &plugin.display_name),
    ];

    if !available_apps.is_empty() {
        lines.push(messages.search_tools.clone());
    }

    if plugin.has_skills {
        let skill_namespace = plugin
            .plugin_namespace
            .as_deref()
            .unwrap_or(plugin.display_name.as_str());
        lines.push(
            messages
                .skill_namespace
                .replace("{{ skill_namespace }}", skill_namespace),
        );
    }

    if !available_apps.is_empty() {
        lines.push(
            messages.apps.replace(
                "{{ apps }}",
                &available_apps
                    .iter()
                    .map(|app| format!("`{app}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        );
    }

    if !available_mcp_servers.is_empty() {
        lines.push(
            messages.mcp_servers.replace(
                "{{ mcp_servers }}",
                &available_mcp_servers
                    .iter()
                    .map(|server| format!("`{server}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        );
    }

    if lines.len() == 1 {
        return None;
    }

    lines.push(messages.closing.clone());

    // The separator belongs to the harness; preserve the catalog's exact bytes after it.
    let omitted = if messages.omitted.starts_with('\n') || messages.omitted.starts_with("\r\n") {
        messages.omitted.clone()
    } else {
        format!("\n{}", messages.omitted)
    };

    Some(bound_explicit_plugin_instructions(
        lines.join("\n"),
        &omitted,
    ))
}

fn bound_explicit_plugin_instructions(rendered: String, truncation_suffix: &str) -> String {
    if rendered.len() <= MAX_EXPLICIT_PLUGIN_INSTRUCTIONS_BYTES {
        return rendered;
    }

    let max_prefix_bytes =
        MAX_EXPLICIT_PLUGIN_INSTRUCTIONS_BYTES.saturating_sub(truncation_suffix.len());
    let prefix = take_bytes_at_char_boundary(&rendered, max_prefix_bytes);
    format!("{prefix}{truncation_suffix}")
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
