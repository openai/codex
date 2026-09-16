//! Purely omits checklist-tool guidance from Codex-owned prompts, before adding caller text.

/// Call only for Codex-owned prompt text; custom instructions must remain unchanged.
pub fn without_update_plan_instructions(instructions: &str) -> String {
    let lines = instructions.split_inclusive('\n').collect::<Vec<_>>();
    let mut rendered = String::with_capacity(instructions.len());
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim_end();
        if matches!(
            line,
            "## Planning"
                | "## `update_plan`"
                | "## Plan tool"
                | "## Plan Mode vs update_plan tool"
        ) {
            let end = (index + 1..lines.len())
                .find(|&next| lines[next].starts_with("# ") || lines[next].starts_with("## "))
                .unwrap_or(lines.len());
            let is_checklist_section = line != "## Planning"
                || lines[index..end].iter().any(|line| {
                    line.starts_with("You have access to an `update_plan` tool")
                        || line.starts_with("When `update_plan` is available, follow this section")
                });
            if is_checklist_section {
                index = end;
                continue;
            }
        }

        if line == "Progress visibility:"
            && lines
                .get(index + 1)
                .is_some_and(|line| line.starts_with("If update_plan is available"))
        {
            index += 2;
            if lines.get(index).is_some_and(|line| line.trim().is_empty()) {
                index += 1;
            }
            continue;
        }

        if line.starts_with("- Use the plan tool ")
            || line.starts_with("- If you create a checklist or task list,")
        {
            index += 1;
            while index < lines.len()
                && (lines[index].starts_with(' ') || lines[index].starts_with('\t'))
            {
                index += 1;
            }
            continue;
        }

        rendered.push_str(lines[index]);
        index += 1;
    }
    rendered
}

#[cfg(test)]
#[path = "update_plan_instructions_tests.rs"]
mod tests;
