//! Resolves bounded plugin wording while leaving capability facts and eligibility to callers.
//! Missing, blank, oversized, or invalid fields independently retain their bundled wording.

use super::ResolvedMessage;
use super::ResolvedModelMessages;
use codex_protocol::openai_models::PluginMessages;

const MAX_WORDING_BYTES: usize = 500;
const MAX_USAGE_BYTES: usize = 2_000;
const GENERIC_INTRO: &str = "A plugin is a local bundle of skills, MCP servers, and apps.";
const GENERIC_USAGE: &str = r###"- Skill naming: If a plugin contributes skills, those skill entries are prefixed with `plugin_name:` in the Skills list.
- MCP naming: Plugin-provided MCP tools keep standard MCP identifiers such as `mcp__server__tool`; use tool provenance to tell which plugin they come from.
- Trigger rules: If the user explicitly names a plugin, prefer capabilities associated with that plugin for that turn.
- Relationship to capabilities: Plugins are not invoked directly. Use their underlying skills, MCP tools, and app tools to help solve the task.
- Relevance: Determine what a plugin can help with from explicit user mention or from the plugin-associated skills, MCP tools, and apps exposed elsewhere in this turn.
- Missing/blocked: If the user requests a plugin that does not have relevant callable capabilities for the task, say so briefly and continue with the best fallback."###;
const SELECTED_INTRO: &str = "Capabilities from the `{{ plugin_name }}` plugin:";
const SEARCH_TOOLS: &str = concat!(
    "- For the user request that explicitly selected this plugin, and only for that ",
    "request, if `tool_search` is available and an app from this plugin may help, ",
    "search for its tools before falling back to unrelated or built-in tools."
);
const SKILL_NAMESPACE: &str =
    "- Skills from this plugin are prefixed with `{{ skill_namespace }}:`.";
const APPS: &str = "- Apps from this plugin available in this session: {{ apps }}.";
const MCP_SERVERS: &str =
    "- MCP servers from this plugin available in this session: {{ mcp_servers }}.";
const CLOSING: &str = "Use these plugin-associated capabilities to help solve the task.";
const OMITTED: &str = "\n- Additional plugin capabilities omitted to fit the context limit.";
const RECOMMENDATIONS_INTRO: &str =
    "Here is a list of plugins that are available but not installed.";

/// Generic wording and its source, retained by the usage-guidance lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedGenericPluginMessages {
    pub intro: String,
    pub usage_instructions: String,
    pub catalog_override: bool,
}

/// Validated templates retain one required placeholder for each harness-owned fact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSelectedPluginMessages {
    pub intro: String,
    pub search_tools: String,
    pub skill_namespace: String,
    pub apps: String,
    pub mcp_servers: String,
    pub closing: String,
    pub omitted: String,
}

/// Bounded introduction independent of recommendation eligibility and plugin identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPluginRecommendationMessages {
    pub intro: String,
}

impl<'a> ResolvedModelMessages<'a> {
    fn plugin_messages(self) -> Option<&'a PluginMessages> {
        self.catalog_messages?.plugins.as_ref()
    }

    /// Resolves the generic plugin guidance used in the conversation's world state.
    pub fn generic_plugin(self) -> ResolvedGenericPluginMessages {
        let generic = self
            .plugin_messages()
            .and_then(|messages| messages.generic.as_ref());
        let intro = literal(
            generic.and_then(|messages| messages.intro.as_deref()),
            GENERIC_INTRO,
            MAX_WORDING_BYTES,
        );
        let usage = literal(
            generic.and_then(|messages| messages.usage_instructions.as_deref()),
            GENERIC_USAGE,
            MAX_USAGE_BYTES,
        );
        ResolvedGenericPluginMessages {
            intro: intro.text().to_owned(),
            usage_instructions: usage.text().to_owned(),
            catalog_override: intro.catalog_override().is_some()
                || usage.catalog_override().is_some(),
        }
    }

    /// Resolves the wording surrounding an explicitly selected plugin's capabilities.
    pub fn selected_plugin(self) -> ResolvedSelectedPluginMessages {
        let selected = self
            .plugin_messages()
            .and_then(|messages| messages.selected.as_ref());
        ResolvedSelectedPluginMessages {
            intro: template(
                selected.and_then(|messages| messages.intro.as_deref()),
                SELECTED_INTRO,
                "{{ plugin_name }}",
            ),
            search_tools: literal(
                selected.and_then(|messages| messages.search_tools.as_deref()),
                SEARCH_TOOLS,
                MAX_WORDING_BYTES,
            )
            .text()
            .to_owned(),
            skill_namespace: template(
                selected.and_then(|messages| messages.skill_namespace.as_deref()),
                SKILL_NAMESPACE,
                "{{ skill_namespace }}",
            ),
            apps: template(
                selected.and_then(|messages| messages.apps.as_deref()),
                APPS,
                "{{ apps }}",
            ),
            mcp_servers: template(
                selected.and_then(|messages| messages.mcp_servers.as_deref()),
                MCP_SERVERS,
                "{{ mcp_servers }}",
            ),
            closing: literal(
                selected.and_then(|messages| messages.closing.as_deref()),
                CLOSING,
                MAX_WORDING_BYTES,
            )
            .text()
            .to_owned(),
            omitted: literal(
                selected.and_then(|messages| messages.omitted.as_deref()),
                OMITTED,
                MAX_WORDING_BYTES,
            )
            .text()
            .to_owned(),
        }
    }

    /// Resolves the introduction to the list of recommended plugins.
    pub fn plugin_recommendations(self) -> ResolvedPluginRecommendationMessages {
        let recommendations = self
            .plugin_messages()
            .and_then(|messages| messages.recommendations.as_ref());
        ResolvedPluginRecommendationMessages {
            intro: literal(
                recommendations.and_then(|messages| messages.intro.as_deref()),
                RECOMMENDATIONS_INTRO,
                MAX_WORDING_BYTES,
            )
            .text()
            .to_owned(),
        }
    }
}

fn literal<'a>(
    catalog: Option<&'a str>,
    bundled: &'static str,
    max_bytes: usize,
) -> ResolvedMessage<'a> {
    ResolvedMessage::new(
        catalog.filter(|text| {
            text.len() <= max_bytes
                && !text.trim().is_empty()
                && !text
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                && ![
                    "<plugins_instructions>",
                    "</plugins_instructions>",
                    "<recommended_plugins>",
                    "</recommended_plugins>",
                ]
                .iter()
                .any(|marker| text.contains(marker))
        }),
        bundled,
    )
}

fn template(catalog: Option<&str>, bundled: &'static str, placeholder: &str) -> String {
    literal(catalog, bundled, MAX_WORDING_BYTES)
        .catalog_override()
        .filter(|text| {
            !text
                .chars()
                .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
                && text.split_once(placeholder).is_some_and(|(before, after)| {
                    ![before, after].iter().any(|part| part.contains(['{', '}']))
                })
        })
        .unwrap_or(bundled)
        .to_owned()
}
