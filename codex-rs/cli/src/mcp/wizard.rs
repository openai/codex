use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::SystemTime;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use inquire::Confirm;
use inquire::Select;
use inquire::Text;
use inquire::validator::ErrorMessage;
use inquire::validator::Validation;

use codex_core::config_types::McpAuthConfig;
use codex_core::config_types::McpHealthcheckConfig;
use codex_core::config_types::McpServerConfig;
use codex_core::mcp::registry::McpRegistry;
use codex_core::mcp::registry::validate_server_name;

use crate::mcp::cli::WizardArgs;

const AUTH_TYPES: &[&str] = &["none", "env", "apikey", "oauth"];
const HEALTH_TYPES: &[&str] = &["none", "stdio", "http"];

#[derive(Debug, Clone)]
pub struct WizardOutcome {
    pub name: String,
    pub server: McpServerConfig,
    pub template_id: Option<String>,
    pub generated_at: SystemTime,
}

impl WizardOutcome {
    pub fn summary(&self) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert("name".into(), self.name.clone());
        map.insert("command".into(), self.server.command.clone());
        if !self.server.args.is_empty() {
            map.insert("args".into(), self.server.args.join(", "));
        }
        if let Some(env) = self.server.env.as_ref()
            && !env.is_empty()
        {
            map.insert(
                "env".into(),
                env.iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if let Some(timeout) = self.server.startup_timeout_ms {
            map.insert("startup_timeout_ms".into(), timeout.to_string());
        }
        if let Some(template_id) = self.template_id.as_ref() {
            map.insert("template_id".into(), template_id.clone());
        }
        if let Some(description) = self.server.description.as_ref() {
            map.insert("description".into(), description.clone());
        }
        if !self.server.tags.is_empty() {
            map.insert("tags".into(), self.server.tags.join(", "));
        }
        map
    }
}

pub fn build_non_interactive(registry: &McpRegistry, args: &WizardArgs) -> Result<WizardOutcome> {
    let template_result = if let Some(template_id) = args.template.as_ref() {
        let cfg = registry
            .instantiate_template(template_id)
            .with_context(|| format!("Template '{template_id}' not found"))?;
        Some((cfg, Some(template_id.clone())))
    } else {
        None
    };

    let mut server = template_result
        .as_ref()
        .map(|(cfg, _)| cfg.clone())
        .unwrap_or_default();

    if let Some(description) = args.description.as_ref() {
        server.description = Some(description.clone());
    }
    if let Some(command) = args.command.as_ref() {
        server.command = command.clone();
    }
    if !args.args.is_empty() {
        server.args = args.args.clone();
    }
    merge_env(&mut server.env, &args.env);

    if let Some(timeout) = args.startup_timeout_ms {
        server.startup_timeout_ms = Some(timeout);
    }
    if !args.tags.is_empty() {
        server.tags = args.tags.clone();
    }

    if args.auth_type.is_some() || args.auth_secret_ref.is_some() || !args.auth_env.is_empty() {
        let mut auth = server.auth.unwrap_or_default();
        if let Some(kind) = args.auth_type.as_ref() {
            auth.kind = Some(kind.clone());
        }
        if let Some(secret_ref) = args.auth_secret_ref.as_ref() {
            auth.secret_ref = Some(secret_ref.clone());
        }
        merge_env(&mut auth.env, &args.auth_env);
        server.auth = Some(auth);
    }

    if args.health_type.is_some()
        || args.health_command.is_some()
        || !args.health_args.is_empty()
        || args.health_timeout_ms.is_some()
        || args.health_interval_seconds.is_some()
        || args.health_endpoint.is_some()
        || args.health_protocol.is_some()
    {
        let mut health = server.healthcheck.unwrap_or_default();
        if let Some(kind) = args.health_type.as_ref() {
            health.kind = Some(kind.clone());
        }
        if let Some(cmd) = args.health_command.as_ref() {
            health.command = Some(cmd.clone());
        }
        if !args.health_args.is_empty() {
            health.args = args.health_args.clone();
        }
        if let Some(timeout) = args.health_timeout_ms {
            health.timeout_ms = Some(timeout);
        }
        if let Some(interval) = args.health_interval_seconds {
            health.interval_seconds = Some(interval);
        }
        if let Some(endpoint) = args.health_endpoint.as_ref() {
            health.endpoint = Some(endpoint.clone());
        }
        if let Some(protocol) = args.health_protocol.as_ref() {
            health.protocol = Some(protocol.clone());
        }
        server.healthcheck = Some(health);
    }

    if server.command.trim().is_empty() {
        bail!("Missing required --command for MCP server launch");
    }

    let name = args
        .name
        .clone()
        .ok_or_else(|| anyhow!("Non-interactive mode requires --name"))?;
    validate_server_name(&name)?;

    let template_id_for_outcome = template_result.as_ref().and_then(|(_, id)| id.clone());

    Ok(WizardOutcome {
        name,
        server,
        template_id: template_id_for_outcome,
        generated_at: SystemTime::now(),
    })
}

pub fn run_interactive(
    registry: &McpRegistry,
    template_hint: Option<&str>,
) -> Result<WizardOutcome> {
    let mut server = template_hint
        .and_then(|id| registry.instantiate_template(id))
        .unwrap_or_default();

    let template_ids = registry.templates().keys().cloned().collect::<Vec<_>>();

    let chosen_template = if !template_ids.is_empty() {
        let mut options = template_ids;
        options.sort();
        let default_index = template_hint
            .and_then(|hint| options.iter().position(|id| id == hint))
            .unwrap_or(0);
        Select::new("Select template", options)
            .with_starting_cursor(default_index)
            .prompt()
            .ok()
    } else {
        None
    };

    if let Some(template_id) = chosen_template.as_deref()
        && let Some(cfg) = registry.instantiate_template(template_id)
    {
        server = cfg;
    }

    let default_name = template_hint
        .map(sanitize_name)
        .or_else(|| chosen_template.as_deref().map(sanitize_name))
        .unwrap_or_default();

    let name = Text::new("MCP server name")
        .with_initial_value(&default_name)
        .with_validator(
            |input: &str| -> Result<Validation, Box<dyn std::error::Error + Send + Sync>> {
                match validate_server_name(input) {
                    Ok(()) => Ok(Validation::Valid),
                    Err(err) => Ok(Validation::Invalid(ErrorMessage::Custom(err.to_string()))),
                }
            },
        )
        .prompt()
        .map_err(|err| anyhow!("Wizard cancelled: {err}"))?;

    loop {
        let command = Text::new("Launch command (e.g. /usr/bin/node)")
            .with_initial_value(&server.command)
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?;
        if command.trim().is_empty() {
            println!("Command must not be empty.");
            continue;
        }
        server.command = command;
        break;
    }

    server.args = parse_list(
        Text::new("Arguments (comma separated, Enter to skip)")
            .with_initial_value(&server.args.join(","))
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
    );

    server.env = collect_env(server.env.take())?;

    server.startup_timeout_ms = parse_optional_u64(
        Text::new("Startup timeout (ms, Enter to skip)")
            .with_initial_value(
                &server
                    .startup_timeout_ms
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            )
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
    )?;

    server.description = Some(
        Text::new("Description (Enter to leave blank)")
            .with_initial_value(server.description.as_deref().unwrap_or(""))
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
    )
    .filter(|s| !s.is_empty());

    server.tags = parse_list(
        Text::new("Tags (comma separated, Enter to skip)")
            .with_initial_value(&server.tags.join(","))
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
    );

    server.auth = collect_auth(server.auth.take())?;
    server.healthcheck = collect_health(server.healthcheck.take())?;

    Ok(WizardOutcome {
        name,
        server,
        template_id: chosen_template.or(template_hint.map(|s| s.to_string())),
        generated_at: SystemTime::now(),
    })
}

pub fn confirm_apply(outcome: &WizardOutcome) -> Result<bool> {
    println!("Configuration summary:");
    for (key, value) in outcome.summary() {
        println!("  {key}: {value}");
    }
    Confirm::new("Persist changes?")
        .with_default(true)
        .prompt()
        .map_err(|err| anyhow!("Wizard cancelled: {err}"))
}

pub fn render_json_summary(outcome: &WizardOutcome) -> Result<String> {
    let json = serde_json::json!({
        "name": outcome.name,
        "command": outcome.server.command,
        "args": outcome.server.args,
        "env": outcome.server.env,
        "startup_timeout_ms": outcome.server.startup_timeout_ms,
        "description": outcome.server.description,
        "tags": outcome.server.tags,
        "template_id": outcome.template_id,
        "auth": outcome.server.auth,
        "healthcheck": outcome.server.healthcheck,
    });
    Ok(serde_json::to_string_pretty(&json)?)
}

fn collect_env(
    existing: Option<HashMap<String, String>>,
) -> Result<Option<HashMap<String, String>>> {
    let mut env = existing.unwrap_or_default();
    while Confirm::new("Add environment variable?")
        .with_default(false)
        .prompt()
        .map_err(|err| anyhow!("Wizard cancelled: {err}"))?
    {
        let pair = Text::new("Enter KEY=VALUE")
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?;
        let (key, value) = parse_env_pair(&pair)?;
        env.insert(key, value);
    }
    if env.is_empty() {
        Ok(None)
    } else {
        Ok(Some(env))
    }
}

fn collect_auth(existing: Option<McpAuthConfig>) -> Result<Option<McpAuthConfig>> {
    let mut auth = existing.unwrap_or_default();
    let selection = Select::new("Authentication type", AUTH_TYPES.to_vec())
        .with_starting_cursor(match auth.kind.as_deref() {
            Some("env") => 1,
            Some("apikey") => 2,
            Some("oauth") => 3,
            _ => 0,
        })
        .prompt()
        .map_err(|err| anyhow!("Wizard cancelled: {err}"))?;

    if selection == "none" {
        return Ok(None);
    }
    auth.kind = Some(selection.to_string());

    auth.secret_ref = Text::new("Secret ref (Enter to skip)")
        .with_initial_value(auth.secret_ref.as_deref().unwrap_or(""))
        .prompt()
        .map_err(|err| anyhow!("Wizard cancelled: {err}"))?
        .trim()
        .to_owned()
        .into();

    auth.env = collect_env(auth.env)?;
    Ok(Some(auth))
}

fn collect_health(existing: Option<McpHealthcheckConfig>) -> Result<Option<McpHealthcheckConfig>> {
    let mut health = existing.unwrap_or_default();
    let selection = Select::new("Health-check type", HEALTH_TYPES.to_vec())
        .with_starting_cursor(match health.kind.as_deref() {
            Some("stdio") => 1,
            Some("http") => 2,
            _ => 0,
        })
        .prompt()
        .map_err(|err| anyhow!("Wizard cancelled: {err}"))?;

    if selection == "none" {
        return Ok(None);
    }
    health.kind = Some(selection.to_string());

    if selection == "stdio" {
        health.command = Some(
            Text::new("Health command")
                .with_initial_value(health.command.as_deref().unwrap_or(""))
                .prompt()
                .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
        );
        health.args = parse_list(
            Text::new("Health args (comma separated)")
                .with_initial_value(&health.args.join(","))
                .prompt()
                .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
        );
        health.endpoint = None;
        health.protocol = None;
    } else {
        health.endpoint = Some(
            Text::new("Health endpoint")
                .with_initial_value(health.endpoint.as_deref().unwrap_or(""))
                .prompt()
                .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
        )
        .filter(|s| !s.is_empty());
        health.protocol = Some("http".into());
        health.command = None;
        health.args.clear();
    }

    health.timeout_ms = parse_optional_u64(
        Text::new("Health timeout (ms, Enter to skip)")
            .with_initial_value(&health.timeout_ms.map(|v| v.to_string()).unwrap_or_default())
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
    )?;
    health.interval_seconds = parse_optional_u64(
        Text::new("Health interval (s, Enter to skip)")
            .with_initial_value(
                &health
                    .interval_seconds
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            )
            .prompt()
            .map_err(|err| anyhow!("Wizard cancelled: {err}"))?,
    )?;

    Ok(Some(health))
}

fn merge_env(target: &mut Option<HashMap<String, String>>, updates: &[(String, String)]) {
    if updates.is_empty() {
        return;
    }
    let mut map = target.take().unwrap_or_default();
    for (k, v) in updates {
        map.insert(k.clone(), v.clone());
    }
    if map.is_empty() {
        *target = None;
    } else {
        *target = Some(map);
    }
}

fn parse_list(input: String) -> Vec<String> {
    input
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn parse_optional_u64(raw: String) -> Result<Option<u64>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = trimmed.parse::<u64>()?;
    Ok(Some(value))
}

fn parse_env_pair(raw: &str) -> Result<(String, String)> {
    let mut parts = raw.splitn(2, '=');
    let key = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("Entry must be KEY=VALUE"))?;
    let value = parts
        .next()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Entry must be KEY=VALUE"))?;
    Ok((key.to_string(), value))
}

fn sanitize_name(template_id: &str) -> String {
    template_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ConfigToml;
    use codex_core::mcp::templates::TemplateCatalog;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn build_non_interactive_applies_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            tmp.path().to_path_buf(),
        )
        .expect("load config");
        let registry = McpRegistry::new(&config, TemplateCatalog::empty());

        let args = WizardArgs {
            name: Some("demo".to_string()),
            command: Some("/bin/echo".to_string()),
            args: vec!["hello".into(), "world".into()],
            env: vec![("FOO".into(), "BAR".into())],
            startup_timeout_ms: Some(1500),
            description: Some("Example".to_string()),
            tags: vec!["fast".into()],
            ..Default::default()
        };

        let outcome = build_non_interactive(&registry, &args).expect("build outcome");

        assert_eq!("demo", outcome.name);
        assert_eq!("/bin/echo", outcome.server.command);
        assert_eq!(vec!["hello", "world"], outcome.server.args);
        assert_eq!(Some(1500), outcome.server.startup_timeout_ms);
        assert_eq!(Some("Example".to_string()), outcome.server.description);
        assert_eq!(vec!["fast".to_string()], outcome.server.tags);

        let env = outcome.server.env.expect("env");
        assert_eq!(Some(&"BAR".to_string()), env.get("FOO"));
    }
}
