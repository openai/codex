/*Semantic model for Git commands.

This module defines the data structures that represent a parsed `git` command
in a structured, semantic way, rather than as a generic list of tokens.
*/

use serde::Serialize;

/// Represents the recognized subcommands for `git`.
#[derive(Debug, Serialize, PartialEq)]
pub enum GitSubcommand {
    Status,
    Log,
    Diff,
    Show,
    Branch,
    Commit(GitCommitOptions),
    Add,
    Stash,
    Checkout,
    Reset(GitResetOptions),
    Rm,
    Unrecognized,
}

/// Options for the `git commit` subcommand.
#[derive(Debug, Serialize, PartialEq)]
pub struct GitCommitOptions {
    pub message: Option<String>,
}

/// Options for the `git reset` subcommand.
#[derive(Debug, Serialize, PartialEq)]
pub struct GitResetOptions {
    pub hard: bool,
}

/// The top-level structure representing a parsed `git` command.
#[derive(Debug, Serialize, PartialEq)]
pub struct GitCommand {
    pub subcommand: GitSubcommand,
}
