use std::collections::BTreeMap;
use std::collections::HashMap;

use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_core::config_types::McpAuthConfig;
use codex_core::config_types::McpHealthcheckConfig;
use codex_core::config_types::McpServerConfig;
use codex_core::config_types::McpTemplate;
use codex_core::mcp::registry::McpRegistry;
use codex_core::mcp::registry::validate_server_name;
use codex_core::mcp::templates::TemplateCatalog;
use ratatui::style::Stylize;
use ratatui::text::Line;

#[derive(Debug, Clone, Default)]
pub(crate) struct McpWizardDraft {
    pub name: String,
    pub template_id: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub startup_timeout_ms: Option<u64>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub auth: Option<AuthDraft>,
    pub health: Option<HealthDraft>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AuthDraft {
    pub kind: Option<String>,
    pub secret_ref: Option<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HealthDraft {
    pub kind: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub interval_seconds: Option<u64>,
    pub endpoint: Option<String>,
    pub protocol: Option<String>,
}

impl McpWizardDraft {
    pub(crate) fn from_existing(name: String, cfg: &McpServerConfig) -> Self {
        let env_map = cfg
            .env
            .as_ref()
            .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let auth = cfg.auth.as_ref().map(|auth| AuthDraft {
            kind: auth.kind.clone(),
            secret_ref: auth.secret_ref.clone(),
            env: auth
                .env
                .as_ref()
                .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        });

        let health = cfg.healthcheck.as_ref().map(|health| HealthDraft {
            kind: health.kind.clone(),
            command: health.command.clone(),
            args: health.args.clone(),
            timeout_ms: health.timeout_ms,
            interval_seconds: health.interval_seconds,
            endpoint: health.endpoint.clone(),
            protocol: health.protocol.clone(),
        });

        Self {
            name,
            template_id: cfg.template_id.clone(),
            command: cfg.command.clone(),
            args: cfg.args.clone(),
            env: env_map,
            startup_timeout_ms: cfg.startup_timeout_ms,
            description: cfg.description.clone(),
            tags: cfg.tags.clone(),
            auth,
            health,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_server_name(&self.name)?;
        if self.command.trim().is_empty() {
            bail!("Command must not be empty");
        }
        for k in self.env.keys() {
            if k.trim().is_empty() {
                bail!("Environment variable keys must not be empty");
            }
        }
        if let Some(auth) = &self.auth {
            if let Some(kind) = &auth.kind
                && kind.trim().is_empty()
            {
                bail!("Authentication type must not be blank");
            }
            for k in auth.env.keys() {
                if k.trim().is_empty() {
                    bail!("Authentication environment keys must not be empty");
                }
            }
        }
        if let Some(health) = &self.health
            && let Some(kind) = &health.kind
            && kind.trim().is_empty()
        {
            bail!("Health check type must not be blank");
        }
        Ok(())
    }

    pub(crate) fn build_server_config(
        &self,
        templates: &TemplateCatalog,
    ) -> Result<McpServerConfig> {
        self.validate()?;

        let mut server = if let Some(template_id) = self.template_id.as_ref() {
            instantiate_template(templates, template_id)?
        } else {
            McpServerConfig::default()
        };

        if self.template_id.is_some() {
            server.template_id = self.template_id.clone();
        }

        server.command = self.command.clone();
        server.args = self.args.clone();
        server.env = map_opt(&self.env);
        server.startup_timeout_ms = self.startup_timeout_ms;
        server.description = self.description.clone();
        server.tags = self.tags.clone();

        server.auth = self.auth.as_ref().map(|auth| McpAuthConfig {
            kind: auth.kind.clone(),
            secret_ref: auth.secret_ref.clone(),
            env: map_opt(&auth.env),
        });

        server.healthcheck = self.health.as_ref().map(|health| McpHealthcheckConfig {
            kind: health.kind.clone(),
            command: health.command.clone(),
            args: health.args.clone(),
            timeout_ms: health.timeout_ms,
            interval_seconds: health.interval_seconds,
            endpoint: health.endpoint.clone(),
            protocol: health.protocol.clone(),
        });

        if server.command.trim().is_empty() {
            bail!("Command must not be empty");
        }

        Ok(server)
    }

    pub(crate) fn apply_template_config(&mut self, cfg: &McpServerConfig) {
        self.template_id = cfg.template_id.clone();
        self.command = cfg.command.clone();
        self.args = cfg.args.clone();
        self.env = cfg
            .env
            .as_ref()
            .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        self.startup_timeout_ms = cfg.startup_timeout_ms;
        self.description = cfg.description.clone();
        self.tags = cfg.tags.clone();
        self.auth = cfg.auth.as_ref().map(|auth| AuthDraft {
            kind: auth.kind.clone(),
            secret_ref: auth.secret_ref.clone(),
            env: auth
                .env
                .as_ref()
                .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        });
        self.health = cfg.healthcheck.as_ref().map(|health| HealthDraft {
            kind: health.kind.clone(),
            command: health.command.clone(),
            args: health.args.clone(),
            timeout_ms: health.timeout_ms,
            interval_seconds: health.interval_seconds,
            endpoint: health.endpoint.clone(),
            protocol: health.protocol.clone(),
        });
    }

    pub(crate) fn summary_lines(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();
        lines.push("MCP Wizard Summary".bold().into());
        lines.push(Line::from(""));
        lines.push(Line::from(vec!["Name: ".dim(), self.name.clone().into()]));
        if let Some(template) = self.template_id.as_ref() {
            lines.push(Line::from(vec![
                "Template: ".dim(),
                template.clone().into(),
            ]));
        } else {
            lines.push(Line::from(vec!["Template: ".dim(), "manual".into()]));
        }
        lines.push(Line::from(vec![
            "Command: ".dim(),
            self.command.clone().into(),
        ]));
        if !self.args.is_empty() {
            lines.push(Line::from(vec!["Args: ".dim(), self.args.join(" ").into()]));
        }
        if !self.env.is_empty() {
            lines.push("Env:".dim().into());
            for (k, v) in &self.env {
                lines.push(Line::from(vec!["  • ".into(), format!("{k}={v}").into()]));
            }
        }
        if let Some(timeout) = self.startup_timeout_ms {
            lines.push(Line::from(vec![
                "Startup timeout (ms): ".dim(),
                timeout.to_string().into(),
            ]));
        }
        if let Some(desc) = self.description.as_ref() {
            lines.push(Line::from(vec!["Description: ".dim(), desc.clone().into()]));
        }
        if !self.tags.is_empty() {
            lines.push(Line::from(vec![
                "Tags: ".dim(),
                self.tags.join(", ").into(),
            ]));
        }
        if let Some(auth) = &self.auth {
            lines.push("Auth:".dim().into());
            if let Some(kind) = auth.kind.as_ref() {
                lines.push(Line::from(vec!["  • Type: ".into(), kind.clone().into()]));
            }
            if let Some(secret) = auth.secret_ref.as_ref() {
                lines.push(Line::from(vec![
                    "  • Secret: ".into(),
                    secret.clone().into(),
                ]));
            }
            if !auth.env.is_empty() {
                for (k, v) in &auth.env {
                    lines.push(Line::from(vec![
                        "     - ".into(),
                        format!("{k}={v}").into(),
                    ]));
                }
            }
        }
        if let Some(health) = &self.health {
            lines.push("Health:".dim().into());
            if let Some(kind) = health.kind.as_ref() {
                lines.push(Line::from(vec!["  • Type: ".into(), kind.clone().into()]));
            }
            if let Some(cmd) = health.command.as_ref() {
                lines.push(Line::from(vec!["  • Command: ".into(), cmd.clone().into()]));
            }
            if !health.args.is_empty() {
                lines.push(Line::from(vec![
                    "  • Args: ".into(),
                    health.args.join(" ").into(),
                ]));
            }
            if let Some(endpoint) = health.endpoint.as_ref() {
                lines.push(Line::from(vec![
                    "  • Endpoint: ".into(),
                    endpoint.clone().into(),
                ]));
            }
            if let Some(timeout) = health.timeout_ms {
                lines.push(Line::from(vec![
                    "  • Timeout (ms): ".into(),
                    timeout.to_string().into(),
                ]));
            }
            if let Some(interval) = health.interval_seconds {
                lines.push(Line::from(vec![
                    "  • Interval (s): ".into(),
                    interval.to_string().into(),
                ]));
            }
        }
        lines
    }
}

fn map_opt(source: &BTreeMap<String, String>) -> Option<HashMap<String, String>> {
    if source.is_empty() {
        None
    } else {
        Some(source.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}

fn instantiate_template(templates: &TemplateCatalog, id: &str) -> Result<McpServerConfig> {
    match templates.instantiate(id) {
        Some(cfg) => Ok(cfg),
        None => Err(anyhow!("Template '{id}' not found")),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateSummary {
    pub id: String,
    pub summary: Option<String>,
    pub category: Option<String>,
}

impl TemplateSummary {
    pub(crate) fn from_template(id: &str, tpl: &McpTemplate) -> Self {
        Self {
            id: id.to_string(),
            summary: tpl.summary.clone(),
            category: tpl.category.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct McpServerSnapshot {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub template_id: Option<String>,
    pub auth: Option<AuthDraft>,
    pub health: Option<HealthDraft>,
    pub display_name: Option<String>,
    pub category: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub startup_timeout_ms: Option<u64>,
}

impl McpServerSnapshot {
    pub(crate) fn from_config(name: &str, cfg: &McpServerConfig) -> Self {
        let env = cfg
            .env
            .as_ref()
            .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let auth = cfg.auth.as_ref().map(|auth| AuthDraft {
            kind: auth.kind.clone(),
            secret_ref: auth.secret_ref.clone(),
            env: auth
                .env
                .as_ref()
                .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        });
        let health = cfg.healthcheck.as_ref().map(|health| HealthDraft {
            kind: health.kind.clone(),
            command: health.command.clone(),
            args: health.args.clone(),
            timeout_ms: health.timeout_ms,
            interval_seconds: health.interval_seconds,
            endpoint: health.endpoint.clone(),
            protocol: health.protocol.clone(),
        });

        Self {
            name: name.to_string(),
            command: cfg.command.clone(),
            args: cfg.args.clone(),
            env,
            description: cfg.description.clone(),
            tags: cfg.tags.clone(),
            template_id: cfg.template_id.clone(),
            auth,
            health,
            display_name: cfg.display_name.clone(),
            category: cfg.category.clone(),
            metadata: cfg.metadata.clone(),
            startup_timeout_ms: cfg.startup_timeout_ms,
        }
    }

    pub(crate) fn to_config(&self) -> McpServerConfig {
        McpServerConfig {
            display_name: self.display_name.clone(),
            category: self.category.clone(),
            template_id: self.template_id.clone(),
            description: self.description.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            env: map_opt(&self.env),
            startup_timeout_ms: self.startup_timeout_ms,
            auth: self.auth.as_ref().map(|auth| McpAuthConfig {
                kind: auth.kind.clone(),
                secret_ref: auth.secret_ref.clone(),
                env: map_opt(&auth.env),
            }),
            healthcheck: self.health.as_ref().map(|health| McpHealthcheckConfig {
                kind: health.kind.clone(),
                command: health.command.clone(),
                args: health.args.clone(),
                timeout_ms: health.timeout_ms,
                interval_seconds: health.interval_seconds,
                endpoint: health.endpoint.clone(),
                protocol: health.protocol.clone(),
            }),
            tags: self.tags.clone(),
            created_at: None,
            last_verified_at: None,
            metadata: self.metadata.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct McpManagerState {
    pub servers: Vec<McpServerSnapshot>,
    pub template_count: usize,
}

impl McpManagerState {
    pub(crate) fn from_registry(registry: &McpRegistry) -> Self {
        let mut servers: Vec<McpServerSnapshot> = registry
            .servers()
            .map(|(name, cfg)| McpServerSnapshot::from_config(name, cfg))
            .collect();
        servers.sort_by(|a, b| a.name.cmp(&b.name));

        let template_count = registry.templates().len();
        Self {
            servers,
            template_count,
        }
    }
}

pub(crate) fn template_summaries(catalog: &TemplateCatalog) -> Vec<TemplateSummary> {
    let mut summaries: Vec<TemplateSummary> = catalog
        .templates()
        .iter()
        .map(|(id, tpl)| TemplateSummary::from_template(id, tpl))
        .collect();
    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    summaries
}
