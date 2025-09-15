#!/usr/bin/env python3

with open('codex-rs/tui/src/chatwidget.rs', 'r') as f:
    lines = f.readlines()

# Find where to insert Context handler
insert_idx = -1
for i, line in enumerate(lines):
    if 'SlashCommand::Status =>' in line:
        # Find the closing brace for Status
        for j in range(i+1, len(lines)):
            if lines[j].strip() == '}':
                insert_idx = j + 1
                break
        break

if insert_idx > 0:
    # Insert Context handler
    lines.insert(insert_idx, '            SlashCommand::Context => {\n')
    lines.insert(insert_idx + 1, '                self.add_context_output();\n')
    lines.insert(insert_idx + 2, '            }\n')

# Find where to add the add_context_output method
method_idx = -1
for i, line in enumerate(lines):
    if 'pub(crate) fn add_status_output' in line:
        # Find the closing brace for this method
        brace_count = 0
        for j in range(i, len(lines)):
            if '{' in lines[j]:
                brace_count += lines[j].count('{')
            if '}' in lines[j]:
                brace_count -= lines[j].count('}')
            if brace_count == 0 and j > i:
                method_idx = j + 1
                break
        break

if method_idx > 0:
    # Insert add_context_output method
    lines.insert(method_idx, '\n')
    lines.insert(method_idx + 1, '    pub(crate) fn add_context_output(&mut self) {\n')
    lines.insert(method_idx + 2, '        // For now, this is a placeholder implementation that shows basic context info\n')
    lines.insert(method_idx + 3, '        // This will be expanded to show detailed context window usage\n')
    lines.insert(method_idx + 4, '        let default_usage;\n')
    lines.insert(method_idx + 5, '        let usage_ref = if let Some(ti) = &self.token_info {\n')
    lines.insert(method_idx + 6, '            &ti.total_token_usage\n')
    lines.insert(method_idx + 7, '        } else {\n')
    lines.insert(method_idx + 8, '            default_usage = TokenUsage::default();\n')
    lines.insert(method_idx + 9, '            &default_usage\n')
    lines.insert(method_idx + 10, '        };\n')
    lines.insert(method_idx + 11, '        self.add_to_history(history_cell::new_context_output(\n')
    lines.insert(method_idx + 12, '            &self.config,\n')
    lines.insert(method_idx + 13, '            usage_ref,\n')
    lines.insert(method_idx + 14, '            &self.conversation_id,\n')
    lines.insert(method_idx + 15, '        ));\n')
    lines.insert(method_idx + 16, '    }\n')

with open('codex-rs/tui/src/chatwidget.rs', 'w') as f:
    f.writelines(lines)

print('Successfully updated chatwidget.rs')