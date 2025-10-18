/*Approval rules for Git commands.

This module takes a structured `GitCommand` and classifies it into a `CommandCategory`.
*/

use super::command_rules::CommandCategory;
use crate::approval::git_model::GitCommand;
use crate::approval::git_model::GitSubcommand;

pub fn classify_git_command(command: &GitCommand) -> CommandCategory {
    match &command.subcommand {
        // Read-only VCS operations
        GitSubcommand::Status
        | GitSubcommand::Log
        | GitSubcommand::Diff
        | GitSubcommand::Show
        | GitSubcommand::Branch => CommandCategory::ReadsVcs,

        // Modifying VCS operations
        GitSubcommand::Commit(_)
        | GitSubcommand::Add
        | GitSubcommand::Stash
        | GitSubcommand::Checkout => CommandCategory::ModifiesVcs,

        // Deletion operations
        GitSubcommand::Reset(opts) => {
            if opts.hard {
                CommandCategory::DeletesData
            } else {
                CommandCategory::ModifiesVcs
            }
        }
        GitSubcommand::Rm => CommandCategory::DeletesData,

        // Unrecognized git commands are still just unrecognized commands.
        GitSubcommand::Unrecognized => CommandCategory::Unrecognized,
    }
}
