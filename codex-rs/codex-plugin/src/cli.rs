//! CLI command parsing for plugin management.
//!
//! Parses `/plugin` subcommands and their arguments.

use crate::installer::PluginSource;
use crate::registry::InstallScope;
use std::path::PathBuf;

/// Parsed plugin command.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginCommand {
    /// Install a plugin.
    Install {
        /// Plugin identifier (name or source).
        source: String,
        /// Marketplace name (defaults to "official").
        marketplace: Option<String>,
        /// Installation scope.
        scope: InstallScope,
        /// Force reinstall even if already installed.
        force: bool,
    },
    /// Uninstall a plugin.
    Uninstall {
        /// Plugin ID (name@marketplace).
        plugin_id: String,
        /// Installation scope to remove from.
        scope: Option<InstallScope>,
    },
    /// Enable a plugin.
    Enable {
        /// Plugin ID (name@marketplace).
        plugin_id: String,
    },
    /// Disable a plugin.
    Disable {
        /// Plugin ID (name@marketplace).
        plugin_id: String,
    },
    /// List installed plugins.
    List {
        /// Filter by scope.
        scope: Option<InstallScope>,
        /// Show all details.
        verbose: bool,
    },
    /// Validate a plugin directory.
    Validate {
        /// Path to plugin directory (default: current directory).
        path: Option<PathBuf>,
    },
    /// Update a plugin to the latest version.
    Update {
        /// Plugin ID (name@marketplace).
        plugin_id: String,
        /// Installation scope to update.
        scope: Option<InstallScope>,
    },
    /// Marketplace management.
    Marketplace(MarketplaceCommand),
    /// Show help.
    Help,
}

/// Marketplace subcommands.
#[derive(Debug, Clone, PartialEq)]
pub enum MarketplaceCommand {
    /// Add a marketplace.
    Add {
        /// Marketplace name.
        name: String,
        /// Marketplace source URL or path.
        source: String,
    },
    /// Remove a marketplace.
    Remove {
        /// Marketplace name.
        name: String,
    },
    /// List configured marketplaces.
    List,
    /// Update marketplace catalog.
    Update {
        /// Marketplace name (all if None).
        name: Option<String>,
    },
}

/// Parse error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

/// Parse a plugin command string.
///
/// # Examples
///
/// ```
/// use codex_plugin::cli::parse_command;
///
/// let cmd = parse_command("install my-plugin").unwrap();
/// let cmd = parse_command("uninstall my-plugin@official").unwrap();
/// let cmd = parse_command("enable my-plugin@official").unwrap();
/// let cmd = parse_command("list --scope user").unwrap();
/// ```
pub fn parse_command(input: &str) -> Result<PluginCommand, ParseError> {
    let input = input.trim();

    if input.is_empty() {
        return Ok(PluginCommand::Help);
    }

    let parts: Vec<&str> = input.split_whitespace().collect();
    let (cmd, args) = parts
        .split_first()
        .ok_or_else(|| ParseError::new("Empty command"))?;

    match *cmd {
        "install" | "i" => parse_install(args),
        "uninstall" | "remove" | "u" | "rm" => parse_uninstall(args),
        "enable" | "on" => parse_enable(args),
        "disable" | "off" => parse_disable(args),
        "list" | "ls" | "l" => parse_list(args),
        "validate" | "check" => parse_validate(args),
        "update" | "up" => parse_update(args),
        "marketplace" | "mp" => parse_marketplace(args),
        "help" | "-h" | "--help" => Ok(PluginCommand::Help),
        _ => Err(ParseError::new(format!(
            "Unknown command: '{}'. Use 'help' to see available commands.",
            cmd
        ))),
    }
}

fn parse_install(args: &[&str]) -> Result<PluginCommand, ParseError> {
    if args.is_empty() {
        return Err(ParseError::new(
            "Usage: plugin install <source> [--marketplace <name>] [--scope user|project] [--force]",
        ));
    }

    let mut source = None;
    let mut marketplace = None;
    let mut scope = InstallScope::User;
    let mut force = false;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--marketplace" | "-m" => {
                i += 1;
                if i >= args.len() {
                    return Err(ParseError::new("--marketplace requires a value"));
                }
                marketplace = Some(args[i].to_string());
            }
            "--scope" | "-s" => {
                i += 1;
                if i >= args.len() {
                    return Err(ParseError::new("--scope requires a value"));
                }
                scope = parse_scope(args[i])?;
            }
            "--force" | "-f" => {
                force = true;
            }
            arg if arg.starts_with('-') => {
                return Err(ParseError::new(format!("Unknown flag: {arg}")));
            }
            arg => {
                if source.is_none() {
                    source = Some(arg.to_string());
                } else {
                    return Err(ParseError::new(format!("Unexpected argument: {arg}")));
                }
            }
        }
        i += 1;
    }

    let source = source.ok_or_else(|| ParseError::new("Missing plugin source"))?;

    Ok(PluginCommand::Install {
        source,
        marketplace,
        scope,
        force,
    })
}

fn parse_uninstall(args: &[&str]) -> Result<PluginCommand, ParseError> {
    if args.is_empty() {
        return Err(ParseError::new(
            "Usage: plugin uninstall <plugin-id> [--scope user|project]",
        ));
    }

    let mut plugin_id = None;
    let mut scope = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--scope" | "-s" => {
                i += 1;
                if i >= args.len() {
                    return Err(ParseError::new("--scope requires a value"));
                }
                scope = Some(parse_scope(args[i])?);
            }
            arg if arg.starts_with('-') => {
                return Err(ParseError::new(format!("Unknown flag: {arg}")));
            }
            arg => {
                if plugin_id.is_none() {
                    plugin_id = Some(arg.to_string());
                } else {
                    return Err(ParseError::new(format!("Unexpected argument: {arg}")));
                }
            }
        }
        i += 1;
    }

    let plugin_id = plugin_id.ok_or_else(|| ParseError::new("Missing plugin ID"))?;

    Ok(PluginCommand::Uninstall { plugin_id, scope })
}

fn parse_enable(args: &[&str]) -> Result<PluginCommand, ParseError> {
    if args.is_empty() {
        return Err(ParseError::new("Usage: plugin enable <plugin-id>"));
    }

    if args.len() > 1 {
        return Err(ParseError::new(format!("Unexpected argument: {}", args[1])));
    }

    Ok(PluginCommand::Enable {
        plugin_id: args[0].to_string(),
    })
}

fn parse_disable(args: &[&str]) -> Result<PluginCommand, ParseError> {
    if args.is_empty() {
        return Err(ParseError::new("Usage: plugin disable <plugin-id>"));
    }

    if args.len() > 1 {
        return Err(ParseError::new(format!("Unexpected argument: {}", args[1])));
    }

    Ok(PluginCommand::Disable {
        plugin_id: args[0].to_string(),
    })
}

fn parse_list(args: &[&str]) -> Result<PluginCommand, ParseError> {
    let mut scope = None;
    let mut verbose = false;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--scope" | "-s" => {
                i += 1;
                if i >= args.len() {
                    return Err(ParseError::new("--scope requires a value"));
                }
                scope = Some(parse_scope(args[i])?);
            }
            "--verbose" | "-v" => {
                verbose = true;
            }
            arg if arg.starts_with('-') => {
                return Err(ParseError::new(format!("Unknown flag: {arg}")));
            }
            arg => {
                return Err(ParseError::new(format!("Unexpected argument: {arg}")));
            }
        }
        i += 1;
    }

    Ok(PluginCommand::List { scope, verbose })
}

fn parse_validate(args: &[&str]) -> Result<PluginCommand, ParseError> {
    let path = args.first().map(|p| PathBuf::from(p));
    Ok(PluginCommand::Validate { path })
}

fn parse_update(args: &[&str]) -> Result<PluginCommand, ParseError> {
    if args.is_empty() {
        return Err(ParseError::new(
            "Usage: plugin update <plugin-id> [--scope user|project]",
        ));
    }

    let mut plugin_id = None;
    let mut scope = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--scope" | "-s" => {
                i += 1;
                if i >= args.len() {
                    return Err(ParseError::new("--scope requires a value"));
                }
                scope = Some(parse_scope(args[i])?);
            }
            arg if arg.starts_with('-') => {
                return Err(ParseError::new(format!("Unknown flag: {arg}")));
            }
            arg => {
                if plugin_id.is_none() {
                    plugin_id = Some(arg.to_string());
                } else {
                    return Err(ParseError::new(format!("Unexpected argument: {arg}")));
                }
            }
        }
        i += 1;
    }

    let plugin_id = plugin_id.ok_or_else(|| ParseError::new("Missing plugin ID"))?;

    Ok(PluginCommand::Update { plugin_id, scope })
}

fn parse_marketplace(args: &[&str]) -> Result<PluginCommand, ParseError> {
    if args.is_empty() {
        return Err(ParseError::new(
            "Usage: plugin marketplace add|remove|list|update [args]",
        ));
    }

    match args[0] {
        "add" => {
            if args.len() < 3 {
                return Err(ParseError::new(
                    "Usage: plugin marketplace add <name> <source>",
                ));
            }
            Ok(PluginCommand::Marketplace(MarketplaceCommand::Add {
                name: args[1].to_string(),
                source: args[2].to_string(),
            }))
        }
        "remove" | "rm" => {
            if args.len() < 2 {
                return Err(ParseError::new("Usage: plugin marketplace remove <name>"));
            }
            Ok(PluginCommand::Marketplace(MarketplaceCommand::Remove {
                name: args[1].to_string(),
            }))
        }
        "list" | "ls" => Ok(PluginCommand::Marketplace(MarketplaceCommand::List)),
        "update" => {
            let name = args.get(1).map(|s| s.to_string());
            Ok(PluginCommand::Marketplace(MarketplaceCommand::Update {
                name,
            }))
        }
        cmd => Err(ParseError::new(format!(
            "Unknown marketplace command: '{}'. Use 'add', 'remove', 'list', or 'update'.",
            cmd
        ))),
    }
}

fn parse_scope(s: &str) -> Result<InstallScope, ParseError> {
    match s.to_lowercase().as_str() {
        "user" | "u" => Ok(InstallScope::User),
        "project" | "p" => Ok(InstallScope::Project),
        "managed" | "m" => Ok(InstallScope::Managed),
        "local" | "l" => Ok(InstallScope::Local),
        _ => Err(ParseError::new(format!(
            "Invalid scope: '{}'. Use 'user', 'project', 'managed', or 'local'.",
            s
        ))),
    }
}

/// Generate help text for plugin commands.
pub fn help_text() -> &'static str {
    r#"Plugin Management Commands:

USAGE:
    plugin <command> [options]

COMMANDS:
    install <source>      Install a plugin from source
        --marketplace <name>  Target marketplace (default: official)
        --scope <scope>       Installation scope: user|project|managed|local
        --force               Reinstall even if already installed

    uninstall <plugin-id>  Remove an installed plugin
        --scope <scope>       Scope to uninstall from (default: all)

    enable <plugin-id>     Enable a disabled plugin
    disable <plugin-id>    Disable a plugin

    list                   List installed plugins
        --scope <scope>       Filter by scope
        --verbose             Show detailed information

    validate [path]        Validate a plugin directory
                           Uses current directory if no path given

    update <plugin-id>     Update a plugin to the latest version
        --scope <scope>       Scope to update (default: all)

    marketplace            Manage marketplaces
        add <name> <url>      Add a new marketplace
        remove <name>         Remove a marketplace
        list                  List configured marketplaces
        update [name]         Update marketplace catalog(s)

    help                   Show this help message

EXAMPLES:
    plugin install my-plugin
    plugin install owner/repo --marketplace github
    plugin install ./local-plugin --scope project
    plugin uninstall my-plugin@official
    plugin enable my-plugin@official
    plugin list --scope user
    plugin validate ./my-plugin

SOURCES:
    Local path:    ./path/to/plugin or ../plugin
    GitHub:        owner/repo
    Git URL:       https://github.com/owner/repo.git
    NPM package:   npm:package-name or npm:@scope/package
"#
}

/// Infer plugin ID from source if not explicitly provided.
///
/// For sources like "owner/repo", extracts "repo" as the plugin name.
pub fn infer_plugin_name(source: &str) -> Option<String> {
    // Try parsing as PluginSource
    if let Ok(parsed) = PluginSource::parse(source) {
        match parsed {
            PluginSource::GitHub { repo, .. } => {
                // Extract repo name from "owner/repo"
                repo.split('/').last().map(|s| s.to_string())
            }
            PluginSource::Git { url, .. } => {
                // Extract from URL like "https://github.com/owner/repo.git"
                url.trim_end_matches(".git")
                    .split('/')
                    .last()
                    .map(|s| s.to_string())
            }
            PluginSource::Local { path } => {
                // Use directory name
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            }
            PluginSource::Npm { package, .. } => {
                // Use package name (strip scope if present)
                if package.starts_with('@') {
                    package.split('/').last().map(|s| s.to_string())
                } else {
                    Some(package)
                }
            }
            PluginSource::Pip { package, .. } => {
                // Use package name as-is
                Some(package)
            }
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_install() {
        let cmd = parse_command("install my-plugin").unwrap();
        assert!(matches!(cmd, PluginCommand::Install { source, .. } if source == "my-plugin"));

        let cmd = parse_command("install owner/repo --marketplace github").unwrap();
        if let PluginCommand::Install {
            source,
            marketplace,
            ..
        } = cmd
        {
            assert_eq!(source, "owner/repo");
            assert_eq!(marketplace, Some("github".to_string()));
        } else {
            panic!("Expected Install command");
        }

        let cmd = parse_command("install ./local --scope project --force").unwrap();
        if let PluginCommand::Install { scope, force, .. } = cmd {
            assert_eq!(scope, InstallScope::Project);
            assert!(force);
        } else {
            panic!("Expected Install command");
        }
    }

    #[test]
    fn test_parse_uninstall() {
        let cmd = parse_command("uninstall my-plugin@official").unwrap();
        if let PluginCommand::Uninstall { plugin_id, scope } = cmd {
            assert_eq!(plugin_id, "my-plugin@official");
            assert!(scope.is_none());
        } else {
            panic!("Expected Uninstall command");
        }

        let cmd = parse_command("rm my-plugin@mp --scope user").unwrap();
        if let PluginCommand::Uninstall { plugin_id, scope } = cmd {
            assert_eq!(plugin_id, "my-plugin@mp");
            assert_eq!(scope, Some(InstallScope::User));
        } else {
            panic!("Expected Uninstall command");
        }
    }

    #[test]
    fn test_parse_enable_disable() {
        let cmd = parse_command("enable test@mp").unwrap();
        assert!(matches!(cmd, PluginCommand::Enable { plugin_id } if plugin_id == "test@mp"));

        let cmd = parse_command("disable test@mp").unwrap();
        assert!(matches!(cmd, PluginCommand::Disable { plugin_id } if plugin_id == "test@mp"));
    }

    #[test]
    fn test_parse_list() {
        let cmd = parse_command("list").unwrap();
        assert!(matches!(
            cmd,
            PluginCommand::List {
                scope: None,
                verbose: false
            }
        ));

        let cmd = parse_command("ls --scope user -v").unwrap();
        if let PluginCommand::List { scope, verbose } = cmd {
            assert_eq!(scope, Some(InstallScope::User));
            assert!(verbose);
        } else {
            panic!("Expected List command");
        }
    }

    #[test]
    fn test_parse_validate() {
        let cmd = parse_command("validate").unwrap();
        assert!(matches!(cmd, PluginCommand::Validate { path: None }));

        let cmd = parse_command("validate ./my-plugin").unwrap();
        if let PluginCommand::Validate { path } = cmd {
            assert_eq!(path, Some(PathBuf::from("./my-plugin")));
        } else {
            panic!("Expected Validate command");
        }
    }

    #[test]
    fn test_parse_marketplace() {
        let cmd = parse_command("marketplace list").unwrap();
        assert!(matches!(
            cmd,
            PluginCommand::Marketplace(MarketplaceCommand::List)
        ));

        let cmd = parse_command("mp add custom https://example.com").unwrap();
        if let PluginCommand::Marketplace(MarketplaceCommand::Add { name, source }) = cmd {
            assert_eq!(name, "custom");
            assert_eq!(source, "https://example.com");
        } else {
            panic!("Expected Marketplace Add command");
        }
    }

    #[test]
    fn test_parse_errors() {
        assert!(parse_command("unknown").is_err());
        assert!(parse_command("install").is_err());
        assert!(parse_command("install foo --unknown").is_err());
        assert!(parse_command("enable").is_err());
    }

    #[test]
    fn test_infer_plugin_name() {
        assert_eq!(infer_plugin_name("owner/repo"), Some("repo".to_string()));
        assert_eq!(
            infer_plugin_name("./my-plugin"),
            Some("my-plugin".to_string())
        );
        assert_eq!(
            infer_plugin_name("npm:@scope/package"),
            Some("package".to_string())
        );
        assert_eq!(
            infer_plugin_name("npm:simple-pkg"),
            Some("simple-pkg".to_string())
        );
    }

    #[test]
    fn test_help() {
        let cmd = parse_command("").unwrap();
        assert!(matches!(cmd, PluginCommand::Help));

        let cmd = parse_command("help").unwrap();
        assert!(matches!(cmd, PluginCommand::Help));

        let cmd = parse_command("--help").unwrap();
        assert!(matches!(cmd, PluginCommand::Help));
    }
}
