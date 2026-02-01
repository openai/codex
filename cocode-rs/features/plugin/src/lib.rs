//! Plugin system for cocode-rs.
//!
//! This crate implements a plugin system that allows extending cocode with:
//! - Custom skills (slash commands)
//! - Custom hooks (lifecycle interceptors)
//! - Custom agents (specialized subagents)
//! - Custom commands (plugin-provided commands)
//! - MCP servers (Model Context Protocol servers)
//!
//! # Architecture
//!
//! Plugins are discovered from multiple scopes in priority order:
//! 1. **Managed** - System-installed plugins
//! 2. **User** - User-global plugins (`~/.config/cocode/plugins/`)
//! 3. **Project** - Project-local plugins (`.cocode/plugins/`)
//!
//! Each plugin contains a `PLUGIN.toml` manifest that declares its contributions.
//!
//! # Plugin Manifest
//!
//! ```toml
//! [plugin]
//! name = "my-plugin"
//! version = "0.1.0"
//! description = "My custom plugin"
//!
//! [contributions]
//! skills = ["skills/"]     # Directories containing SKILL.toml files
//! hooks = ["hooks.toml"]   # Hook configuration files
//! agents = ["agents/"]     # Directories containing AGENT.toml files
//! commands = ["commands/"] # Directories containing COMMAND.toml files
//! mcp_servers = ["mcp/"]   # Directories containing MCP.toml files
//! ```

pub mod agent_loader;
pub mod command;
pub mod command_loader;
pub mod contribution;
pub mod integration;
pub mod loader;
pub mod manifest;
pub mod mcp;
pub mod mcp_loader;
pub mod registry;
pub mod scope;

mod error;

// Re-export primary types
pub use command::CommandHandler;
pub use command::PluginCommand;
pub use contribution::PluginContribution;
pub use contribution::PluginContributions;
pub use error::PluginError;
pub use error::Result;
pub use integration::PluginIntegrationConfig;
pub use integration::integrate_plugins;
pub use integration::load_plugins;
pub use loader::LoadedPlugin;
pub use loader::PluginLoader;
pub use loader::load_plugins_from_roots;
pub use manifest::PluginManifest;
pub use mcp::McpServerConfig;
pub use mcp::McpTransport;
pub use registry::PluginRegistry;
pub use scope::PluginScope;
