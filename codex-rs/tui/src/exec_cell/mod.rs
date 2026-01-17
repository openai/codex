//! Re-exports exec cell data and rendering helpers for the TUI.
//!
//! An exec cell represents a shell or tool invocation and its streamed output
//! as rendered in the transcript. This module is a narrow façade that gathers
//! the data model from [`model`] and the rendering helpers from [`render`], so
//! other modules can import the exec cell API without depending on submodules
//! directly.
//!
//! The re-exports here are crate-private to keep the exec cell implementation
//! cohesive while still allowing the rest of the TUI to build rows, compute
//! line limits, and create live “active command” placeholders during streaming.

mod model;
mod render;

/// Captures the formatted output associated with an exec cell.
pub(crate) use model::CommandOutput;
/// Exposes exec call fixtures for exec cell tests.
#[cfg(test)]
pub(crate) use model::ExecCall;
/// Owns the state and metadata needed to render a single exec cell.
pub(crate) use model::ExecCell;
/// Bundles parameters that control output line shaping and truncation.
pub(crate) use render::OutputLinesParams;
/// Maximum number of lines rendered for tool call output.
pub(crate) use render::TOOL_CALL_MAX_LINES;
/// Builds a transient exec cell for a currently running command.
pub(crate) use render::new_active_exec_command;
/// Converts an exec cell into displayable output lines.
pub(crate) use render::output_lines;
/// Produces a spinner glyph used for running exec cells.
pub(crate) use render::spinner;
