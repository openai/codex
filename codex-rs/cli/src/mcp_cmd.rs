use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_overrides;
use codex_core::config_types::McpServerConfig;
use codex_core::git_info::resolve_root_git_project_for_trust;
use codex_core::mcp_toml::McpToml;
use codex_core::mcp_toml::McpTomlEntry;
use codex_core::mcp_toml::load_project_overlays;
use codex_core::mcp_toml::to_mcp_server_config;
use serde_json::json;
use tempfile as _;
use toml::Value as TomlValue;
use toml_edit as _; // ensure dependency is linked

#[derive(Debug, Parser)]
#[command(
    about = "Manage MCP servers and run Codex as an MCP server",
    long_about = "Manage Model Context Protocol (MCP) servers configured for Codex.\n\nUse subcommands to add, import, list, inspect, or remove servers.\nIf no subcommand is provided, this runs the built-in MCP server (back-compat).",
    after_help = "Examples:\n  # Add a local stdio server (everything after -- is the server command)\n  codex mcp add airtable --env AIRTABLE_API_KEY=YOUR_KEY -- npx -y airtable-mcp-server\n\n  # Import multiple servers from a TOML file into project scope\n  codex mcp add-toml --scope project ./mcp.toml\n\n  # List configured servers (merged view with precedence local > project > user)\n  codex mcp list --json\n\n  # Show details for a specific server\n  codex mcp get airtable --json\n\n  # Remove a server from the user scope\n  codex mcp remove airtable --scope user\n\n  # Remove a server from all scopes\n  codex mcp remove airtable --all\n\n  # Windows: wrap npx with cmd /c\n  codex mcp add my-svc -- cmd /c npx -y @some/package"
)]
pub struct McpCli {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub cmd: Option<McpSub>,
}

#[derive(Debug, clap::Subcommand)]
pub enum McpSub {
    /// Run Codex as an MCP server (back-compat: `codex mcp`).
    Serve,
    /// List configured MCP servers (merged view).
    List {
        #[arg(long)]
        json: bool,
    },
    /// Get details for a specific server name (merged view).
    Get {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Add an MCP stdio server entry to a given scope.
    Add(AddArgs),
    /// Remove an MCP server entry from a given scope or all scopes.
    Remove(RemoveArgs),
    /// Import one or more MCP servers from a TOML file with a [mcp_servers] table.
    AddToml(AddTomlArgs),
}

pub async fn run_main(mcp_cli: McpCli, codex_linux_sandbox_exe: Option<PathBuf>) -> Result<()> {
    match mcp_cli.cmd.unwrap_or(McpSub::Serve) {
        McpSub::Serve => {
            // Preserve the historical `codex mcp` behavior.
            codex_mcp_server::run_main(codex_linux_sandbox_exe, mcp_cli.config_overrides).await?
        }
        McpSub::List { json } => {
            list_servers(mcp_cli.config_overrides, json)?;
        }
        McpSub::Get { name, json } => {
            get_server(mcp_cli.config_overrides, &name, json)?;
        }
        McpSub::Add(args) => {
            add_server(mcp_cli.config_overrides, args)?;
        }
        McpSub::Remove(args) => {
            remove_server(mcp_cli.config_overrides, args)?;
        }
        McpSub::AddToml(args) => {
            add_toml(mcp_cli.config_overrides, args)?;
        }
    }
    Ok(())
}

fn parse_cli_overrides(overrides: CliConfigOverrides) -> Vec<(String, TomlValue)> {
    overrides.parse_overrides().unwrap_or_default()
}

fn load_user_project_local_maps(
    cli_overrides: CliConfigOverrides,
) -> Result<(
    HashMap<String, McpServerConfig>,
    HashMap<String, McpServerConfig>,
    HashMap<String, McpServerConfig>,
)> {
    // User map via `~/.codex/config.toml` (+ -c overrides)
    let codex_home = find_codex_home()?;
    let user_cfg =
        load_config_as_toml_with_cli_overrides(&codex_home, parse_cli_overrides(cli_overrides))?;
    let mut user_map = user_cfg.mcp_servers;

    // Project/local overlays via current project root
    let cwd = std::env::current_dir()?;
    let project_root = resolve_root_git_project_for_trust(&cwd).unwrap_or(cwd);
    let overlays = load_project_overlays(&project_root)?;

    let mut project_map = HashMap::new();
    let mut local_map = HashMap::new();
    for (scope, overlay) in overlays {
        for (name, entry) in overlay.mcp_servers.into_iter() {
            // Convert permissive overlay entry → strict config, expanding env vars.
            if let Ok(cfg) = to_mcp_server_config(&entry, |k| std::env::var(k).ok()) {
                match scope {
                    codex_core::mcp_toml::Scope::Project => {
                        project_map.insert(name, cfg);
                    }
                    codex_core::mcp_toml::Scope::Local => {
                        local_map.insert(name, cfg);
                    }
                    codex_core::mcp_toml::Scope::User => {
                        user_map.insert(name, cfg);
                    }
                }
            }
        }
    }

    Ok((user_map, project_map, local_map))
}

fn list_servers(cli_overrides: CliConfigOverrides, json_out: bool) -> Result<()> {
    let (user_map, project_map, local_map) = load_user_project_local_maps(cli_overrides)?;
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(user_map.keys().cloned());
    names.extend(project_map.keys().cloned());
    names.extend(local_map.keys().cloned());

    if json_out {
        let mut arr = Vec::new();
        for name in names {
            let (scope, cfg, shadowed_by) =
                pick_with_scope(&name, &user_map, &project_map, &local_map);
            arr.push(json!({
                "name": name,
                "scope": scope,
                "config": cfg_to_json(cfg),
                "shadowed_by": shadowed_by,
            }));
        }
        println!("{}", serde_json::to_string_pretty(&arr)?);
    } else {
        for name in names {
            let (scope, cfg, _) = pick_with_scope(&name, &user_map, &project_map, &local_map);
            let args_preview = if cfg.args.is_empty() {
                String::new()
            } else {
                format!(" {}", cfg.args.join(" "))
            };
            println!("{} [{}] -> {}{}", name, scope, cfg.command, args_preview);
        }
    }
    Ok(())
}

fn get_server(cli_overrides: CliConfigOverrides, name: &str, json_out: bool) -> Result<()> {
    let (user_map, project_map, local_map) = load_user_project_local_maps(cli_overrides)?;
    if !user_map.contains_key(name)
        && !project_map.contains_key(name)
        && !local_map.contains_key(name)
    {
        anyhow::bail!("MCP server `{}` not found in any scope", name);
    }
    let (scope, cfg, shadowed_by) = pick_with_scope(name, &user_map, &project_map, &local_map);
    if json_out {
        let obj = json!({
            "name": name,
            "scope": scope,
            "config": cfg_to_json(cfg),
            "shadowed_by": shadowed_by,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        let args_preview = if cfg.args.is_empty() {
            String::new()
        } else {
            format!(" {}", cfg.args.join(" "))
        };
        println!("{} [{}] -> {}{}", name, scope, cfg.command, args_preview);
    }
    Ok(())
}

fn pick_with_scope<'a>(
    name: &str,
    user_map: &'a HashMap<String, McpServerConfig>,
    project_map: &'a HashMap<String, McpServerConfig>,
    local_map: &'a HashMap<String, McpServerConfig>,
) -> (&'static str, &'a McpServerConfig, Vec<&'static str>) {
    if let Some(cfg) = local_map.get(name) {
        (
            "local",
            cfg,
            vec![
                if project_map.contains_key(name) {
                    "project"
                } else {
                    ""
                },
                if user_map.contains_key(name) {
                    "user"
                } else {
                    ""
                },
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
        )
    } else if let Some(cfg) = project_map.get(name) {
        (
            "project",
            cfg,
            vec![if user_map.contains_key(name) {
                "user"
            } else {
                ""
            }]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
        )
    } else if let Some(cfg) = user_map.get(name) {
        ("user", cfg, vec![])
    } else {
        // Should not occur because callers pre-check membership. Return a
        // fallback to avoid panics in release builds.
        let fallback = user_map
            .iter()
            .next()
            .or_else(|| project_map.iter().next())
            .or_else(|| local_map.iter().next());
        let (k, v) = match fallback {
            Some(kv) => kv,
            None => panic!("internal error: no MCP server entries found across scopes"),
        };
        let _ = k; // suppress unused warning
        ("user", v, vec![])
    }
}

fn cfg_to_json(cfg: &McpServerConfig) -> serde_json::Value {
    json!({
        "command": cfg.command,
        "args": cfg.args,
        "env": cfg.env,
    })
}

// ------------------------------
// Add/remove writers
// ------------------------------

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
enum ScopeArg {
    Local,
    Project,
    User,
}

#[derive(Debug, Parser)]
pub struct AddArgs {
    /// Unique server name (^[A-Za-z0-9_-]+$)
    name: String,
    /// Target scope
    #[arg(long, value_enum, default_value_t = ScopeArg::Local)]
    scope: ScopeArg,
    /// Environment variables KEY=VALUE (repeatable)
    #[arg(long = "env")]
    env: Vec<String>,
    /// Command and args to launch the MCP server (after `--`)
    #[arg(trailing_var_arg = true)]
    cmd: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct RemoveArgs {
    /// Server name
    name: String,
    /// Scope to remove from; omit with --all to remove everywhere
    #[arg(long, value_enum)]
    scope: Option<ScopeArg>,
    /// Remove from all scopes
    #[arg(long)]
    all: bool,
}

fn add_server(cli_overrides: CliConfigOverrides, args: AddArgs) -> Result<()> {
    validate_server_name(&args.name)?;
    if args.cmd.is_empty() {
        anyhow::bail!(
            "missing server command; use: codex mcp add <name> [--scope ...] [--env KEY=VALUE]... -- <command> [args...]"
        );
    }
    let command = args.cmd[0].clone();
    let cmd_args: Vec<String> = args.cmd.iter().skip(1).cloned().collect();
    let env_map = parse_env_kv(args.env.iter())?;

    let path = match args.scope {
        ScopeArg::User => {
            write_user_scope(&args.name, &command, &cmd_args, &env_map, cli_overrides)?
        }
        ScopeArg::Project => write_overlay_scope(&args.name, &command, &cmd_args, &env_map, false)?,
        ScopeArg::Local => write_overlay_scope(&args.name, &command, &cmd_args, &env_map, true)?,
    };
    println!(
        "Added MCP server '{}' (scope: {}) → wrote {}",
        args.name,
        match args.scope {
            ScopeArg::Local => "local",
            ScopeArg::Project => "project",
            ScopeArg::User => "user",
        },
        path.display()
    );
    Ok(())
}

fn remove_server(cli_overrides: CliConfigOverrides, args: RemoveArgs) -> Result<()> {
    if args.all && args.scope.is_some() {
        anyhow::bail!("cannot use --scope with --all");
    }

    if args.all {
        let u = remove_user_scope(&args.name, cli_overrides.clone())?;
        if u.wrote {
            println!("Removed '{}' → wrote {}", args.name, u.path.display());
        }
        let p = remove_overlay_scope(&args.name, false)?;
        if p.wrote {
            println!("Removed '{}' → wrote {}", args.name, p.path.display());
        }
        let l = remove_overlay_scope(&args.name, true)?;
        if l.wrote {
            println!("Removed '{}' → wrote {}", args.name, l.path.display());
        }
        return Ok(());
    }

    let outcome = match args.scope.unwrap_or(ScopeArg::Local) {
        ScopeArg::User => remove_user_scope(&args.name, cli_overrides)?,
        ScopeArg::Project => remove_overlay_scope(&args.name, false)?,
        ScopeArg::Local => remove_overlay_scope(&args.name, true)?,
    };
    if outcome.wrote {
        println!("Removed '{}' → wrote {}", args.name, outcome.path.display());
    } else {
        println!(
            "No changes for '{}' at {}",
            args.name,
            outcome.path.display()
        );
    }
    Ok(())
}

#[derive(Debug, Parser)]
pub struct AddTomlArgs {
    /// Path to a TOML file containing a [mcp_servers] table
    path: PathBuf,
    /// Target scope to import into
    #[arg(long, value_enum, default_value_t = ScopeArg::Local)]
    scope: ScopeArg,
}

fn add_toml(_cli_overrides: CliConfigOverrides, args: AddTomlArgs) -> Result<()> {
    let contents = std::fs::read_to_string(&args.path)?;
    let parsed: McpToml = toml::from_str(&contents)?;
    let mut accepted: Vec<(String, McpTomlEntry)> = Vec::new();
    let mut rejected: Vec<(String, String)> = Vec::new();
    for (name, entry) in parsed.mcp_servers.into_iter() {
        if let Some(t) = entry.r#type.as_deref()
            && !t.eq_ignore_ascii_case("stdio")
        {
            rejected.push((name, format!("unsupported transport `{}`", t)));
            continue;
        }
        if entry.command.is_none() {
            rejected.push((name, "missing command".to_string()));
            continue;
        }
        accepted.push((name, entry));
    }

    let path = match args.scope {
        ScopeArg::User => write_user_batch(&accepted)?,
        ScopeArg::Project => write_overlay_batch(&accepted, false)?,
        ScopeArg::Local => write_overlay_batch(&accepted, true)?,
    };
    println!(
        "Imported {} MCP server(s) into {}",
        accepted.len(),
        path.display()
    );

    if !rejected.is_empty() {
        for (n, why) in rejected {
            eprintln!("skipped `{}`: {}", n, why);
        }
    }
    Ok(())
}

fn parse_env_kv<'a>(pairs: impl Iterator<Item = &'a String>) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for p in pairs {
        if let Some((k, v)) = p.split_once('=') {
            if k.is_empty() {
                anyhow::bail!("invalid --env '{}': empty key", p);
            }
            map.insert(k.to_string(), v.to_string());
        } else {
            anyhow::bail!("invalid --env '{}': expected KEY=VALUE", p);
        }
    }
    Ok(map)
}

fn validate_server_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        anyhow::bail!(
            "invalid server name '{}': must match ^[a-zA-Z0-9_-]+$",
            name
        )
    }
}

fn resolve_codex_home_for_write() -> Result<PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME")
        && !val.is_empty()
    {
        let p = PathBuf::from(val);
        if !p.exists() {
            std::fs::create_dir_all(&p)?;
        }
        return Ok(p.canonicalize().unwrap_or(p));
    }
    let p = find_codex_home()?;
    if !p.exists() {
        std::fs::create_dir_all(&p)?;
    }
    Ok(p)
}

fn write_user_scope(
    name: &str,
    command: &str,
    args: &[String],
    env_map: &HashMap<String, String>,
    cli_overrides: CliConfigOverrides,
) -> Result<PathBuf> {
    let codex_home = resolve_codex_home_for_write()?;
    let path = codex_home.join("config.toml");
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = contents
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    upsert_mcp_entry(&mut doc, name, command, args, env_map);
    write_doc_atomic(&doc, &path)?;
    let _ = cli_overrides;
    Ok(path)
}

fn write_overlay_scope(
    name: &str,
    command: &str,
    args: &[String],
    env_map: &HashMap<String, String>,
    local: bool,
) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let project_root = resolve_root_git_project_for_trust(&cwd).unwrap_or(cwd);
    let fname = if local {
        ".mcp.local.toml"
    } else {
        ".mcp.toml"
    };
    let path = project_root.join(fname);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = contents
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    upsert_mcp_entry(&mut doc, name, command, args, env_map);
    write_doc_atomic(&doc, &path)?;
    Ok(path)
}

fn write_user_batch(entries: &[(String, McpTomlEntry)]) -> Result<PathBuf> {
    let codex_home = resolve_codex_home_for_write()?;
    let path = codex_home.join("config.toml");
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = contents
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    for (name, entry) in entries {
        let args = entry.args.clone();
        let env_map = entry.env.clone();
        let command = entry.command.clone().unwrap_or_default();
        upsert_mcp_entry(&mut doc, name, &command, &args, &env_map);
    }
    write_doc_atomic(&doc, &path)?;
    Ok(path)
}

fn write_overlay_batch(entries: &[(String, McpTomlEntry)], local: bool) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let project_root = resolve_root_git_project_for_trust(&cwd).unwrap_or(cwd);
    let fname = if local {
        ".mcp.local.toml"
    } else {
        ".mcp.toml"
    };
    let path = project_root.join(fname);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = contents
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    for (name, entry) in entries {
        let args = entry.args.clone();
        let env_map = entry.env.clone();
        let command = entry.command.clone().unwrap_or_default();
        upsert_mcp_entry(&mut doc, name, &command, &args, &env_map);
    }
    write_doc_atomic(&doc, &path)?;
    Ok(path)
}

struct RemoveOutcome {
    path: PathBuf,
    wrote: bool,
}

fn remove_user_scope(name: &str, _cli_overrides: CliConfigOverrides) -> Result<RemoveOutcome> {
    let codex_home = resolve_codex_home_for_write()?;
    let path = codex_home.join("config.toml");
    if !path.exists() {
        return Ok(RemoveOutcome { path, wrote: false });
    }
    let contents = std::fs::read_to_string(&path)?;
    let mut doc = contents.parse::<toml_edit::DocumentMut>()?;
    if let Some(tbl) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_mut()) {
        if tbl.remove(name).is_some() {
            write_doc_atomic(&doc, &path)?;
            return Ok(RemoveOutcome { path, wrote: true });
        }
    }
    Ok(RemoveOutcome { path, wrote: false })
}

fn remove_overlay_scope(name: &str, local: bool) -> Result<RemoveOutcome> {
    let cwd = std::env::current_dir()?;
    let project_root = resolve_root_git_project_for_trust(&cwd).unwrap_or(cwd);
    let fname = if local {
        ".mcp.local.toml"
    } else {
        ".mcp.toml"
    };
    let path = project_root.join(fname);
    if !path.exists() {
        return Ok(RemoveOutcome { path, wrote: false });
    }
    let contents = std::fs::read_to_string(&path)?;
    let mut doc = contents.parse::<toml_edit::DocumentMut>()?;
    if let Some(tbl) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_mut()) {
        if tbl.remove(name).is_some() {
            write_doc_atomic(&doc, &path)?;
            return Ok(RemoveOutcome { path, wrote: true });
        }
    }
    Ok(RemoveOutcome { path, wrote: false })
}

fn upsert_mcp_entry(
    doc: &mut toml_edit::DocumentMut,
    name: &str,
    command: &str,
    args: &[String],
    env_map: &HashMap<String, String>,
) {
    if !doc.as_table().contains_key("mcp_servers") {
        doc.insert("mcp_servers", toml_edit::table());
    }
    let tbl = doc["mcp_servers"].as_table_mut().expect("table");
    tbl.set_implicit(false);

    if !tbl.contains_key(name) {
        tbl.insert(name, toml_edit::table());
    }
    let st = tbl[name].as_table_mut().expect("subtable");
    st.set_implicit(false);

    st["command"] = toml_edit::value(command);
    let mut arr = toml_edit::Array::new();
    for a in args {
        arr.push(a.as_str());
    }
    st["args"] = toml_edit::Item::Value(toml_edit::Value::Array(arr));

    if env_map.is_empty() {
        if st.contains_key("env") {
            st.remove("env");
        }
    } else {
        let mut kv = toml_edit::InlineTable::new();
        for (k, v) in env_map {
            kv.get_or_insert(k, toml_edit::Value::from(v.as_str()));
        }
        st["env"] = toml_edit::Item::Value(toml_edit::Value::InlineTable(kv));
    }
}

fn write_doc_atomic(doc: &toml_edit::DocumentMut, path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tempfile::NamedTempFile::new_in(
        path.parent().unwrap_or_else(|| std::path::Path::new(".")),
    )?;
    std::fs::write(tmp.path(), doc.to_string())?;
    tmp.persist(path)?;
    Ok(())
}
