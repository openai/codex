// Deprecated compatibility layer for the old plan_* names. These are todo list
// types and not related to Plan mode.
#[deprecated(note = "use codex_protocol::todo_tool::TodoItemArg")]
pub use crate::todo_tool::TodoItemArg as PlanItemArg;
#[deprecated(note = "use codex_protocol::todo_tool::TodoStatus")]
pub use crate::todo_tool::TodoStatus as StepStatus;
#[deprecated(note = "use codex_protocol::todo_tool::UpdateTodoArgs")]
pub use crate::todo_tool::UpdateTodoArgs as UpdatePlanArgs;
