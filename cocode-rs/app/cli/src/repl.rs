//! Interactive REPL for chat sessions.

use cocode_session::SessionState;
use cocode_skill::SkillManager;
use cocode_skill::execute_skill;
use std::io::BufRead;
use std::io::Write;
use std::io::{self};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::output;

/// Interactive REPL for chat sessions.
pub struct Repl<'a> {
    session: &'a mut SessionState,
    /// Skill manager for handling slash commands.
    skill_manager: Arc<Mutex<SkillManager>>,
}

impl<'a> Repl<'a> {
    /// Create a new REPL with the given session.
    pub fn new(session: &'a mut SessionState) -> Self {
        Self {
            session,
            skill_manager: Arc::new(Mutex::new(SkillManager::new())),
        }
    }

    /// Create a new REPL with a skill manager.
    pub fn with_skill_manager(
        session: &'a mut SessionState,
        skill_manager: Arc<Mutex<SkillManager>>,
    ) -> Self {
        Self {
            session,
            skill_manager,
        }
    }

    /// Run the interactive REPL loop.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        output::print_session_start(
            self.session.session_id(),
            self.session.model(),
            self.session.provider(),
        );

        loop {
            // Print prompt
            print!("> ");
            io::stdout().flush()?;

            // Read input
            let input = match self.read_input() {
                Ok(Some(input)) => input,
                Ok(None) => {
                    // EOF - exit
                    println!();
                    println!("Goodbye!");
                    break;
                }
                Err(e) => {
                    output::print_error(&e.to_string());
                    continue;
                }
            };

            // Skip empty input
            if input.trim().is_empty() {
                continue;
            }

            // Handle commands
            if input.starts_with('/') {
                if self.handle_command(&input).await? {
                    // Command requested exit
                    break;
                }
                continue;
            }

            // Run the turn
            match self.session.run_turn(&input).await {
                Ok(result) => {
                    // Print the response
                    println!();
                    println!("{}", result.final_text);
                    output::print_turn_summary(
                        result.usage.input_tokens,
                        result.usage.output_tokens,
                    );
                    println!();
                }
                Err(e) => {
                    output::print_error(&e.to_string());
                }
            }
        }

        Ok(())
    }

    /// Read a line of input from stdin.
    fn read_input(&self) -> anyhow::Result<Option<String>> {
        let stdin = io::stdin();
        let mut line = String::new();
        let bytes = stdin.lock().read_line(&mut line)?;

        if bytes == 0 {
            // EOF
            return Ok(None);
        }

        Ok(Some(line.trim().to_string()))
    }

    /// Handle a / command.
    ///
    /// Returns true if the REPL should exit.
    async fn handle_command(&mut self, input: &str) -> anyhow::Result<bool> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts.first().map(|s| *s).unwrap_or("");

        match cmd {
            "/exit" | "/quit" | "/q" => {
                println!("Goodbye!");
                return Ok(true);
            }
            "/help" | "/h" | "/?" => {
                self.print_help().await;
            }
            "/skills" => {
                self.list_skills().await;
            }
            "/status" => {
                println!("Session ID: {}", self.session.session_id());
                println!(
                    "Model:      {}/{}",
                    self.session.provider(),
                    self.session.model()
                );
                println!("Turns:      {}", self.session.total_turns());
                println!(
                    "Tokens:     {} in / {} out",
                    self.session.total_input_tokens(),
                    self.session.total_output_tokens()
                );
            }
            "/clear" => {
                // Clear screen using ANSI escape codes
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush()?;
            }
            "/cancel" => {
                self.session.cancel();
                println!("Operation cancelled.");
            }
            _ => {
                // Try to execute as a skill command
                if self.try_execute_skill(input).await? {
                    // Skill was executed - result already printed
                } else {
                    println!("Unknown command: {cmd}");
                    println!(
                        "Type /help for available commands or /skills to list available skills."
                    );
                }
            }
        }

        Ok(false)
    }

    /// Print help including available skills.
    async fn print_help(&self) {
        println!("Commands:");
        println!("  /help, /h, /?  - Show this help");
        println!("  /exit, /quit   - Exit the chat");
        println!("  /status        - Show session status");
        println!("  /skills        - List available skills");
        println!("  /clear         - Clear the screen");
        println!("  /cancel        - Cancel current operation");

        let manager = self.skill_manager.lock().await;
        if !manager.is_empty() {
            println!();
            println!("Skills:");
            for skill in manager.all() {
                println!("  /{} - {}", skill.name, skill.description);
            }
        }
    }

    /// List all available skills.
    async fn list_skills(&self) {
        let manager = self.skill_manager.lock().await;
        if manager.is_empty() {
            println!("No skills loaded.");
            println!("Skills are loaded from .cocode/skills/ directories.");
        } else {
            println!("Available skills ({}):", manager.len());
            for skill in manager.all() {
                println!("  /{} - {}", skill.name, skill.description);
            }
        }
    }

    /// Try to execute a skill command.
    ///
    /// Returns true if a skill was found and executed.
    async fn try_execute_skill(&mut self, input: &str) -> anyhow::Result<bool> {
        let manager = self.skill_manager.lock().await;
        let result = execute_skill(&manager, input);
        drop(manager); // Release lock before running turn

        match result {
            Some(skill_result) => {
                println!("Executing skill: /{}", skill_result.skill_name);
                println!();

                // Run the skill prompt as a turn
                match self.session.run_turn(&skill_result.prompt).await {
                    Ok(result) => {
                        println!("{}", result.final_text);
                        output::print_turn_summary(
                            result.usage.input_tokens,
                            result.usage.output_tokens,
                        );
                        println!();
                    }
                    Err(e) => {
                        output::print_error(&e.to_string());
                    }
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }
}
