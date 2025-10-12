/*Parser for Git commands.

This module is responsible for parsing a `git` command (represented as a slice
of strings) into a structured `GitCommand` semantic model.
*/

use super::git_model::GitCommand;
use super::git_model::GitCommitOptions;
use super::git_model::GitResetOptions;
use super::git_model::GitSubcommand;

/// Parses a git command slice into a `GitCommand` struct.
///
/// This is a simplified parser that handles a few common subcommands and options.
pub fn parse_git_command(args: &[String]) -> GitCommand {
    // The first argument after "git" is usually the subcommand.
    let subcommand_str = args.get(1).map(std::string::String::as_str);

    let subcommand = match subcommand_str {
        Some("status") => GitSubcommand::Status,
        Some("log") => GitSubcommand::Log,
        Some("diff") => GitSubcommand::Diff,
        Some("show") => GitSubcommand::Show,
        Some("branch") => GitSubcommand::Branch,
        Some("add") => GitSubcommand::Add,
        Some("stash") => GitSubcommand::Stash,
        Some("checkout") => GitSubcommand::Checkout,
        Some("rm") => GitSubcommand::Rm,
        Some("commit") => {
            let mut message = None;
            if let Some(m_index) = args.iter().position(|r| r == "-m")
                && let Some(msg) = args.get(m_index + 1)
            {
                message = Some(msg.clone());
            }
            GitSubcommand::Commit(GitCommitOptions { message })
        }
        Some("reset") => {
            let hard = args.iter().any(|arg| arg == "--hard");
            GitSubcommand::Reset(GitResetOptions { hard })
        }
        _ => GitSubcommand::Unrecognized,
    };

    GitCommand { subcommand }
}
