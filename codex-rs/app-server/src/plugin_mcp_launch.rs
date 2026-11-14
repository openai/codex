//! Parses the host's process-local launch directories for stdio MCP servers.
//! Validates server names, plugin IDs, and absolute paths before passing the
//! directories to Core. These overrides are never written to shared config.

use std::ffi::OsString;
use std::io;
use std::path::Path;

use clap::Args;
use codex_core::config::McpLaunchCwds;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Launch options for stdio servers in this app-server process.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginMcpLaunchArgs {
    #[arg(long = "plugin-mcp-launch-cwd", hide = true, num_args = 3, value_names = ["PLUGIN_ID", "SERVER", "DIRECTORY"])]
    entries: Vec<OsString>,
    #[arg(long = "mcp-launch-cwd", hide = true, num_args = 3, value_names = ["SERVER", "EXECUTABLE", "DIRECTORY"])]
    configured_entries: Vec<OsString>,
}

impl PluginMcpLaunchArgs {
    pub(crate) fn into_cwds(self) -> io::Result<McpLaunchCwds> {
        let mut cwds = McpLaunchCwds::default();
        for [plugin_id, server, directory] in self.entries.as_chunks::<3>().0 {
            let plugin_id = plugin_id.to_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "plugin ID must be UTF-8")
            })?;
            PluginId::parse(plugin_id)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            let server = server_name(server)?;
            let cwd = AbsolutePathBuf::from_absolute_path_checked(Path::new(directory))?;
            cwds.plugins
                .entry(plugin_id.to_owned())
                .or_default()
                .insert(server.to_owned(), cwd);
        }
        for [server, executable, directory] in self.configured_entries.as_chunks::<3>().0 {
            let server = server_name(server)?;
            let executable = executable.to_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "executable must be UTF-8")
            })?;
            AbsolutePathBuf::from_absolute_path_checked(Path::new(executable))?;
            let cwd = AbsolutePathBuf::from_absolute_path_checked(Path::new(directory))?;
            cwds.configured
                .insert(server.to_owned(), (executable.to_owned(), cwd));
        }
        Ok(cwds)
    }
}

fn server_name(value: &OsString) -> io::Result<&str> {
    value
        .to_str()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "server name must be nonempty UTF-8",
            )
        })
}
