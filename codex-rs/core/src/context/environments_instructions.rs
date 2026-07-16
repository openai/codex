use codex_protocol::protocol::ENVIRONMENTS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;

pub(crate) struct EnvironmentsInstructions;

impl ContextualUserFragment for EnvironmentsInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG,
            ENVIRONMENTS_INSTRUCTIONS_CLOSE_TAG,
        )
    }

    fn body(&self) -> String {
        "\n## Execution environments\n\
Execution environments are separate machines or workspaces with their own files, shell, and installed capabilities. `<environment_context>` lists the environments selected for this task.\n\
\n\
An environment marked `starting` is not yet usable. Its files, commands, AGENTS.md instructions, skills, plugins, and MCP tools may become available when startup completes.\n\
\n\
Wait only when the current task needs that environment. Continue using tools that are already available for unrelated work.\n"
            .to_string()
    }
}
