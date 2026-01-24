//! Command parsing for spawn tasks.
//!
//! Parses command-line style arguments for spawn tasks.
//! Used by both TUI (/spawn command) and exec CLI (--iter/--time flags).

use crate::loop_driver::LoopCondition;

/// Parsed spawn command (subcommand + arguments).
#[derive(Debug, Clone)]
pub enum SpawnCommand {
    /// Start new spawn task (--prompt required).
    Start(SpawnCommandArgs),
    /// List all spawn tasks (--list).
    List,
    /// Get status of a task (--status <task-id>).
    Status { task_id: String },
    /// Kill a running task (--kill <task-id>).
    Kill { task_id: String },
    /// Drop task metadata (--drop <task-id>).
    Drop { task_id: String },
    /// Merge task branches (--merge <task-id>... [--prompt <msg>]).
    Merge {
        task_ids: Vec<String>,
        prompt: Option<String>,
    },
    /// Show help (no args or invalid).
    Help,
}

/// Parsed arguments for starting a spawn task.
#[derive(Debug, Clone, Default)]
pub struct SpawnCommandArgs {
    /// Task name/ID (optional, auto-generated if not provided).
    pub name: Option<String>,
    /// Model override in "provider" or "provider/model" format.
    pub model: Option<String>,
    /// Loop condition (--iter or --time).
    pub loop_condition: Option<LoopCondition>,
    /// Task prompt (everything after --prompt).
    pub prompt: Option<String>,
    /// Skip inheriting parent's plan context (run standalone).
    pub detach: bool,
}

/// Parse spawn command.
///
/// Supported formats:
/// - `/spawn` → Help
/// - `/spawn --list` → List
/// - `/spawn --status <task-id>` → Status
/// - `/spawn --kill <task-id>` → Kill
/// - `/spawn --drop <task-id>` → Drop
/// - `/spawn --merge <task-id>... [--prompt <msg>]` → Merge
/// - `/spawn [--name <id>] [--model <p>] [--iter <n> | --time <d>] --prompt <task>` → Start
pub fn parse_spawn_command(input: &str) -> Result<SpawnCommand, String> {
    let input = input.trim();

    // Strip leading /spawn if present (for TUI command)
    let args_str = input.strip_prefix("/spawn").unwrap_or(input).trim();

    // No args → Help
    if args_str.is_empty() {
        return Ok(SpawnCommand::Help);
    }

    let tokens: Vec<&str> = args_str.split_whitespace().collect();

    // Check for subcommand flags first
    match tokens.first() {
        Some(&"--list") => return Ok(SpawnCommand::List),
        Some(&"--status") => {
            if tokens.len() < 2 {
                return Err("--status requires a task ID".to_string());
            }
            return Ok(SpawnCommand::Status {
                task_id: tokens[1].to_string(),
            });
        }
        Some(&"--kill") => {
            if tokens.len() < 2 {
                return Err("--kill requires a task ID".to_string());
            }
            return Ok(SpawnCommand::Kill {
                task_id: tokens[1].to_string(),
            });
        }
        Some(&"--drop") => {
            if tokens.len() < 2 {
                return Err("--drop requires a task ID".to_string());
            }
            return Ok(SpawnCommand::Drop {
                task_id: tokens[1].to_string(),
            });
        }
        Some(&"--merge") => {
            return parse_merge_command(&tokens[1..]);
        }
        _ => {}
    }

    // Otherwise, parse as Start command
    parse_start_command(&tokens).map(SpawnCommand::Start)
}

/// Parse --merge subcommand.
fn parse_merge_command(tokens: &[&str]) -> Result<SpawnCommand, String> {
    if tokens.is_empty() {
        return Err("--merge requires at least one task ID".to_string());
    }

    let mut task_ids = Vec::new();
    let mut prompt = None;
    let mut i = 0;

    while i < tokens.len() {
        if tokens[i] == "--prompt" {
            i += 1;
            if i >= tokens.len() {
                return Err("--prompt requires a message".to_string());
            }
            // Rest of tokens are the prompt
            prompt = Some(tokens[i..].join(" "));
            break;
        } else if tokens[i].starts_with("--") {
            return Err(format!("Unknown option in merge: {}", tokens[i]));
        } else {
            task_ids.push(tokens[i].to_string());
        }
        i += 1;
    }

    if task_ids.is_empty() {
        return Err("--merge requires at least one task ID".to_string());
    }

    Ok(SpawnCommand::Merge { task_ids, prompt })
}

/// Parse Start command arguments.
fn parse_start_command(tokens: &[&str]) -> Result<SpawnCommandArgs, String> {
    let mut args = SpawnCommandArgs::default();
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i] {
            "--name" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("--name requires a value".to_string());
                }
                args.name = Some(tokens[i].to_string());
            }
            "--model" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("--model requires a value".to_string());
                }
                args.model = Some(tokens[i].to_string());
            }
            "--iter" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("--iter requires a number".to_string());
                }
                let count: i32 = tokens[i]
                    .parse()
                    .map_err(|_| format!("Invalid iteration count: {}", tokens[i]))?;
                if count <= 0 {
                    return Err("Iteration count must be positive".to_string());
                }
                args.loop_condition = Some(LoopCondition::Iters { count });
            }
            "--time" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("--time requires a duration (e.g., 1h, 30m)".to_string());
                }
                args.loop_condition = Some(
                    LoopCondition::parse(tokens[i])
                        .map_err(|e| format!("Invalid duration: {e}"))?,
                );
            }
            "--prompt" => {
                // Everything after --prompt is the task description
                i += 1;
                if i >= tokens.len() {
                    return Err("--prompt requires a task description".to_string());
                }
                let prompt = tokens[i..].join(" ");
                args.prompt = Some(prompt);
                break; // --prompt consumes the rest
            }
            "--detach" => {
                args.detach = true;
            }
            other => {
                return Err(format!("Unknown option: {other}"));
            }
        }
        i += 1;
    }

    // Validate required fields
    if args.prompt.is_none() {
        return Err("Missing --prompt. Specify the task description.".to_string());
    }
    if args.loop_condition.is_none() {
        return Err("Missing --iter or --time. Specify how long to run.".to_string());
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to extract Start args from SpawnCommand.
    fn expect_start(cmd: SpawnCommand) -> SpawnCommandArgs {
        match cmd {
            SpawnCommand::Start(args) => args,
            other => panic!("Expected Start, got {:?}", other),
        }
    }

    #[test]
    fn parse_spawn_help() {
        let result = parse_spawn_command("/spawn").unwrap();
        assert!(matches!(result, SpawnCommand::Help));
    }

    #[test]
    fn parse_spawn_list() {
        let result = parse_spawn_command("/spawn --list").unwrap();
        assert!(matches!(result, SpawnCommand::List));
    }

    #[test]
    fn parse_spawn_status() {
        let result = parse_spawn_command("/spawn --status task-1").unwrap();
        match result {
            SpawnCommand::Status { task_id } => assert_eq!(task_id, "task-1"),
            other => panic!("Expected Status, got {:?}", other),
        }
    }

    #[test]
    fn parse_spawn_kill() {
        let result = parse_spawn_command("/spawn --kill my-task").unwrap();
        match result {
            SpawnCommand::Kill { task_id } => assert_eq!(task_id, "my-task"),
            other => panic!("Expected Kill, got {:?}", other),
        }
    }

    #[test]
    fn parse_spawn_drop() {
        let result = parse_spawn_command("/spawn --drop old-task").unwrap();
        match result {
            SpawnCommand::Drop { task_id } => assert_eq!(task_id, "old-task"),
            other => panic!("Expected Drop, got {:?}", other),
        }
    }

    #[test]
    fn parse_spawn_merge_single() {
        let result = parse_spawn_command("/spawn --merge task-1").unwrap();
        match result {
            SpawnCommand::Merge { task_ids, prompt } => {
                assert_eq!(task_ids, vec!["task-1"]);
                assert!(prompt.is_none());
            }
            other => panic!("Expected Merge, got {:?}", other),
        }
    }

    #[test]
    fn parse_spawn_merge_multiple() {
        let result = parse_spawn_command("/spawn --merge task-1 task-2 task-3").unwrap();
        match result {
            SpawnCommand::Merge { task_ids, prompt } => {
                assert_eq!(task_ids, vec!["task-1", "task-2", "task-3"]);
                assert!(prompt.is_none());
            }
            other => panic!("Expected Merge, got {:?}", other),
        }
    }

    #[test]
    fn parse_spawn_merge_with_prompt() {
        let result = parse_spawn_command("/spawn --merge task-1 --prompt merge with care").unwrap();
        match result {
            SpawnCommand::Merge { task_ids, prompt } => {
                assert_eq!(task_ids, vec!["task-1"]);
                assert_eq!(prompt, Some("merge with care".to_string()));
            }
            other => panic!("Expected Merge, got {:?}", other),
        }
    }

    #[test]
    fn parse_spawn_with_iter() {
        let result = parse_spawn_command("/spawn --iter 5 --prompt implement feature").unwrap();
        let args = expect_start(result);
        assert_eq!(args.loop_condition, Some(LoopCondition::Iters { count: 5 }));
        assert_eq!(args.prompt, Some("implement feature".to_string()));
        assert!(args.name.is_none());
        assert!(args.model.is_none());
    }

    #[test]
    fn parse_spawn_with_all_options() {
        let result = parse_spawn_command(
            "/spawn --name my-task --model DeepSeek --iter 3 --prompt fix all bugs",
        )
        .unwrap();
        let args = expect_start(result);
        assert_eq!(args.name, Some("my-task".to_string()));
        assert_eq!(args.model, Some("DeepSeek".to_string()));
        assert_eq!(args.loop_condition, Some(LoopCondition::Iters { count: 3 }));
        assert_eq!(args.prompt, Some("fix all bugs".to_string()));
    }

    #[test]
    fn parse_spawn_with_model_and_model_name() {
        let result =
            parse_spawn_command("/spawn --model DeepSeek/deepseek-chat --iter 1 --prompt test")
                .unwrap();
        let args = expect_start(result);
        assert_eq!(args.model, Some("DeepSeek/deepseek-chat".to_string()));
    }

    #[test]
    fn parse_spawn_missing_prompt() {
        let result = parse_spawn_command("/spawn --iter 5");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--prompt"));
    }

    #[test]
    fn parse_spawn_missing_loop_condition() {
        let result = parse_spawn_command("/spawn --prompt do something");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--iter"));
    }

    #[test]
    fn parse_spawn_prompt_captures_rest() {
        let result =
            parse_spawn_command("/spawn --iter 1 --prompt implement user auth with OAuth2")
                .unwrap();
        let args = expect_start(result);
        assert_eq!(
            args.prompt,
            Some("implement user auth with OAuth2".to_string())
        );
    }

    #[test]
    fn parse_without_slash_spawn_prefix() {
        let result = parse_spawn_command("--iter 3 --prompt test task").unwrap();
        let args = expect_start(result);
        assert_eq!(args.loop_condition, Some(LoopCondition::Iters { count: 3 }));
        assert_eq!(args.prompt, Some("test task".to_string()));
    }

    #[test]
    fn parse_spawn_status_missing_id() {
        let result = parse_spawn_command("/spawn --status");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("task ID"));
    }

    #[test]
    fn parse_spawn_merge_missing_id() {
        let result = parse_spawn_command("/spawn --merge");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("task ID"));
    }

    #[test]
    fn parse_spawn_with_detach() {
        let result =
            parse_spawn_command("/spawn --detach --iter 3 --prompt implement feature").unwrap();
        let args = expect_start(result);
        assert!(args.detach);
        assert_eq!(args.loop_condition, Some(LoopCondition::Iters { count: 3 }));
        assert_eq!(args.prompt, Some("implement feature".to_string()));
    }

    #[test]
    fn parse_spawn_without_detach() {
        let result = parse_spawn_command("/spawn --iter 3 --prompt test task").unwrap();
        let args = expect_start(result);
        assert!(!args.detach);
    }

    #[test]
    fn parse_spawn_detach_with_all_options() {
        let result = parse_spawn_command(
            "/spawn --name my-task --model DeepSeek --detach --iter 5 --prompt do work",
        )
        .unwrap();
        let args = expect_start(result);
        assert_eq!(args.name, Some("my-task".to_string()));
        assert_eq!(args.model, Some("DeepSeek".to_string()));
        assert!(args.detach);
        assert_eq!(args.loop_condition, Some(LoopCondition::Iters { count: 5 }));
        assert_eq!(args.prompt, Some("do work".to_string()));
    }
}
