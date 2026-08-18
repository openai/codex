use tree_sitter::Node;
use tree_sitter::Parser;

/// Lowers a literal PowerShell script into argv-like command vectors.
///
/// This module does not decide whether a command is safe or dangerous; callers apply those
/// policies to the lowered words. Its job is only to recognize a deliberately small literal
/// PowerShell subset and fail closed for everything else.
///
/// Unknown syntax, parse recovery, and dynamic expressions fail closed instead of being guessed
/// at. The accepted CST shapes intentionally cover only common literal command forms; rare
/// PowerShell syntax and value-conversion cases stay opaque.
pub(crate) fn try_parse_powershell_commands(script: &str) -> Option<Vec<Vec<String>>> {
    lower_with_tree_sitter(script).ok()
}

fn lower_with_tree_sitter(script: &str) -> Result<Vec<Vec<String>>, String> {
    // PowerShell treats these Unicode characters as syntax aliases even when tree-sitter leaves
    // them inside generic tokens. Keep that whole spelling family opaque rather than guessing at
    // whether a quote or dash is structural in a particular position.
    if script
        .chars()
        .any(|ch| matches!(ch, '‘' | '’' | '“' | '”' | '–' | '—' | '―'))
    {
        return Err("PowerShell Unicode syntax alias".to_string());
    }

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_powershell::LANGUAGE.into())
        .map_err(|error| format!("load grammar: {error}"))?;
    // The grammar rejects native `--flag=value`. Mask only the `=` in conservatively
    // recognized bare tokens; the one-byte replacement keeps CST ranges valid for `script`.
    let mut parse_source = script.as_bytes().to_vec();
    for equals in inline_double_dash_parameter_equals(script) {
        parse_source[equals] = b' ';
    }
    let parse_source = String::from_utf8(parse_source)
        .map_err(|_| "masked command source is not UTF-8".to_string())?;
    let tree = parser
        .parse(&parse_source, None)
        .ok_or_else(|| "tree-sitter returned no tree".to_string())?;
    let root = tree.root_node();
    if root.has_error() {
        return Err("tree contains ERROR or missing nodes".to_string());
    }
    if has_requires_directive(root, script) {
        return Err("requires directives can execute before command lowering".to_string());
    }
    if let Some(kind) = first_unrecognized_named_kind(root) {
        return Err(format!("unrecognized named node: {kind}"));
    }
    let mut command_nodes = Vec::new();
    collect_command_nodes(root, &mut command_nodes);
    if command_nodes.is_empty() {
        return Err("no literal command nodes".to_string());
    }

    let mut commands = Vec::with_capacity(command_nodes.len());
    let mut command_ranges = Vec::with_capacity(command_nodes.len());
    for node in command_nodes {
        let text = node
            .utf8_text(script.as_bytes())
            .map_err(|_| "command source is not UTF-8".to_string())?;
        command_ranges.push(node.start_byte()..node.end_byte());
        commands.push(lower_command_text(text)?);
    }
    if !source_is_covered_by_commands(script, &command_ranges) {
        return Err("source outside literal command nodes".to_string());
    }
    if commands.iter().any(|command| {
        command
            .first()
            .is_some_and(|word| word.eq_ignore_ascii_case("using"))
    }) {
        return Err("using declarations require the PowerShell AST oracle".to_string());
    }
    if commands
        .iter()
        .any(|command| command.is_empty() || command.iter().any(String::is_empty))
    {
        return Err("empty lowered command or word".to_string());
    }
    Ok(commands)
}

fn collect_command_nodes<'tree>(root: Node<'tree>, commands: &mut Vec<Node<'tree>>) {
    // Script nesting is model-controlled, so keep CST traversal off the call stack.
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "command" {
            commands.push(node);
            continue;
        }
        for child_index in (0..node.named_child_count()).rev() {
            if let Some(child) = node.named_child(child_index) {
                stack.push(child);
            }
        }
    }
}

fn has_requires_directive(root: Node<'_>, script: &str) -> bool {
    // Tree-sitter exposes #requires as a comment, but PowerShell evaluates it before the
    // script body and can load modules or assemblies.
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "comment"
            && node
                .utf8_text(script.as_bytes())
                .ok()
                .is_some_and(|comment| {
                    comment
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("#requires")
                })
        {
            return true;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    false
}

// These helpers are the allowlist for PowerShell CST forms we intentionally understand. A new
// named tree-sitter node is rejected until its lowering semantics are reviewed.
fn first_unrecognized_named_kind(root: Node<'_>) -> Option<String> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_named() && !is_allowed_named_kind(node.kind()) {
            return Some(node.kind().to_string());
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            stack.push(child);
        }
    }
    None
}

fn is_allowed_named_kind(kind: &str) -> bool {
    matches!(
        kind,
        "program"
            | "statement_list"
            | "pipeline"
            | "pipeline_chain"
            | "pipeline_chain_tail"
            | "command"
            | "command_name"
            | "command_elements"
            | "command_argument_sep"
            | "command_parameter"
            | "generic_token"
            | "array_literal_expression"
            | "unary_expression"
            | "string_literal"
            | "verbatim_string_characters"
            | "expandable_string_literal"
            | "integer_literal"
            | "decimal_integer_literal"
            | "comment"
            | "empty_statement"
            // Negative numeric flags such as git log -1 use this wrapper.
            | "expression_with_unary_operator"
    )
}

// Find only bare literal `--flag=value` forms whose `=` can be masked before parsing.
fn inline_double_dash_parameter_equals(script: &str) -> Vec<usize> {
    let mut equals = Vec::new();
    let mut start = 0;
    for (index, ch) in script.char_indices() {
        if is_inline_parameter_token_separator(ch) {
            push_inline_parameter_equals(&mut equals, script, start, index);
            start = index + ch.len_utf8();
        }
    }
    push_inline_parameter_equals(&mut equals, script, start, script.len());
    equals
}

fn push_inline_parameter_equals(equals: &mut Vec<usize>, script: &str, start: usize, end: usize) {
    let token = &script[start..end];
    if token.starts_with("--")
        && token
            .chars()
            .all(|ch| !is_rejected_inline_parameter_character(ch))
        && let Some(relative_equals) = token.find('=')
        && relative_equals > 2
        && relative_equals + 1 < token.len()
    {
        equals.push(start + relative_equals);
    }
}

fn is_inline_parameter_token_separator(ch: char) -> bool {
    ch.is_whitespace() || "|&;#><(){}".contains(ch)
}

fn is_rejected_inline_parameter_character(ch: char) -> bool {
    ch.is_whitespace() || is_rejected_bare_character(ch) || matches!(ch, '\u{60}' | '#')
}

fn source_is_covered_by_commands(script: &str, command_ranges: &[std::ops::Range<usize>]) -> bool {
    // Command nodes alone are not enough: reject any source outside the literal commands and
    // separators/comments we explicitly understand.
    let mut index = 0;
    let mut range_index = 0;
    let mut can_chain = false;
    let mut needs_command = false;
    let mut paren_depth = 0;
    while index < script.len() {
        if let Some(range) = command_ranges.get(range_index)
            && index == range.start
        {
            index = range.end;
            range_index += 1;
            can_chain = true;
            needs_command = false;
            continue;
        }

        let Some(ch) = script[index..].chars().next() else {
            return false;
        };
        let next = index + ch.len_utf8();
        if ch == '\r' || ch == '\n' {
            can_chain = false;
            index = next;
            continue;
        }
        if ch.is_whitespace() {
            index = next;
            continue;
        }
        if ch == ';' {
            if needs_command {
                return false;
            }
            can_chain = false;
            index = next;
            continue;
        }
        if ch == '(' && !can_chain {
            paren_depth += 1;
            index = next;
            continue;
        }
        if ch == ')' && paren_depth > 0 && !needs_command {
            paren_depth -= 1;
            index = next;
            continue;
        }
        if ch == '|' && can_chain {
            can_chain = false;
            needs_command = true;
            index = if script[next..].starts_with('|') {
                next + '|'.len_utf8()
            } else {
                next
            };
            continue;
        }
        if ch == '&' && can_chain && script[next..].starts_with('&') {
            can_chain = false;
            needs_command = true;
            index = next + '&'.len_utf8();
            continue;
        }
        if ch == '#' && !needs_command {
            // `#` starts a comment only at a token boundary. Tree-sitter can split an
            // embedded `#` out of a bare token, so reject that recovery instead of dropping
            // the rest of the line.
            if index > 0
                && !script[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|previous| previous.is_whitespace() || previous == ';')
            {
                return false;
            }
            index = script[next..]
                .find(['\r', '\n'])
                .map_or(script.len(), |offset| next + offset);
            continue;
        }
        if script[index..].starts_with("<#")
            && !needs_command
            && (index == 0
                || script[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|previous| previous.is_whitespace() || previous == ';'))
        {
            let Some(end) = script[next..].find("#>") else {
                return false;
            };
            index = next + end + "#>".len();
            continue;
        }
        return false;
    }
    range_index == command_ranges.len() && !needs_command && paren_depth == 0
}

fn lower_command_text(command_text: &str) -> Result<Vec<String>, String> {
    // This is literal argv lowering, not safe/dangerous classification. Quoting and escapes are
    // decoded only for forms whose runtime value is statically known.
    let mut words = Vec::new();
    let chars: Vec<char> = command_text.trim().chars().collect();
    let mut index = 0;
    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index == chars.len() || chars[index] == '#' {
            break;
        }
        let (word, next, is_bare) = if chars[index] == '\'' {
            let (word, next) = parse_single_quoted(&chars, index)?;
            (word, next, false)
        } else if chars[index] == '"' {
            let (word, next) = parse_double_quoted(&chars, index)?;
            (word, next, false)
        } else {
            let (word, next) = parse_bare_word(&chars, index)?;
            (word, next, true)
        };
        index = next;
        if index < chars.len() && !chars[index].is_whitespace() && chars[index] != '#' {
            return Err("adjacent/concatenated command elements".to_string());
        }
        if word.is_empty() {
            return Err("empty word".to_string());
        }
        if is_bare {
            reject_unsupported_bare_word(&word)?;
        }
        words.push(word);
    }

    if words.is_empty() {
        return Err("command lowered to no words".to_string());
    }
    Ok(words)
}

fn parse_single_quoted(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let mut value = String::new();
    let mut index = start + 1;
    while index < chars.len() {
        if chars[index] == '\'' {
            if chars.get(index + 1) == Some(&'\'') {
                value.push('\'');
                index += 2;
                continue;
            }
            return Ok((value, index + 1));
        }
        value.push(chars[index]);
        index += 1;
    }
    Err("unterminated single-quoted string".to_string())
}

fn parse_double_quoted(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let mut value = String::new();
    let mut index = start + 1;
    while index < chars.len() {
        match chars[index] {
            '"' => return Ok((value, index + 1)),
            '$' => return Err("expandable string contains variable syntax".to_string()),
            '`' => {
                let escaped = *chars
                    .get(index + 1)
                    .ok_or_else(|| "trailing PowerShell escape".to_string())?;
                // `e is ESC only in PowerShell 6+, so it has no version-neutral value.
                if escaped == 'e' {
                    return Err("PowerShell-version-dependent escape".to_string());
                }
                if escaped == 'u' && chars.get(index + 2) == Some(&'{') {
                    return Err("PowerShell Unicode escape".to_string());
                }
                value.push(decode_backtick_escape(escaped));
                index += 2;
            }
            ch => {
                value.push(ch);
                index += 1;
            }
        }
    }
    Err("unterminated double-quoted string".to_string())
}

fn decode_backtick_escape(ch: char) -> char {
    match ch {
        '0' => '\0',
        'a' => '\u{7}',
        'b' => '\u{8}',
        'f' => '\u{c}',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'v' => '\u{b}',
        other => other,
    }
}

fn parse_bare_word(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let mut value = String::new();
    let mut index = start;
    while index < chars.len() && !chars[index].is_whitespace() {
        let ch = chars[index];
        if ch == '`' {
            let escaped = *chars
                .get(index + 1)
                .ok_or_else(|| "trailing PowerShell escape".to_string())?;
            if escaped == 'e' {
                return Err("PowerShell-version-dependent escape".to_string());
            }
            value.push(decode_backtick_escape(escaped));
            index += 2;
            continue;
        }
        if is_rejected_bare_character(ch) {
            return Err(format!("dynamic or structural bare character: {ch:?}"));
        }
        value.push(ch);
        index += 1;
    }
    if value.is_empty() {
        return Err("empty bare word".to_string());
    }
    Ok((value, index))
}

fn is_rejected_bare_character(ch: char) -> bool {
    matches!(
        ch,
        '$' | '@'
            | '\''
            | '"'
            | '('
            | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | ';'
            | '|'
            | '&'
            | '>'
            | '<'
            | ','
    )
}

fn reject_unsupported_bare_word(word: &str) -> Result<(), String> {
    // These forms require PowerShell-specific value conversion. Keeping them opaque is safer
    // than reproducing that conversion in the policy parser.
    if word.starts_with('-') && !word.starts_with("--") && word.contains(':') {
        return Err("attached PowerShell parameter value".to_string());
    }

    // Tree-sitter leaves some PowerShell numerics as generic tokens. Keep only canonical decimal
    // spellings whose runtime string is obviously identical to the source spelling.
    if word.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && !(word == "0" || (!word.starts_with('0') && word.chars().all(|ch| ch.is_ascii_digit())))
    {
        return Err("non-canonical numeric-leading bare word".to_string());
    }

    Ok(())
}

#[cfg(test)]
#[path = "powershell_tree_sitter_tests.rs"]
mod tests;
