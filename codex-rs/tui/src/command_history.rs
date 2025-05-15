use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use anyhow::Result;

const HISTORY_FILE: &str = ".codex/history.json";
const DEFAULT_HISTORY_SIZE: usize = 10_000;

// Regex patterns for sensitive commands that should not be saved
const SENSITIVE_PATTERNS: &[&str] = &[
    r"\b[A-Za-z0-9-_]{20,}\b", // API keys and tokens
    r"\bpassword\b",
    r"\bsecret\b",
    r"\btoken\b",
    r"\bkey\b",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    command: String,
    timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryConfig {
    max_size: usize,
    save_history: bool,
    sensitive_patterns: Vec<String>,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_HISTORY_SIZE,
            save_history: true,
            sensitive_patterns: Vec::new(),
        }
    }
}

pub struct CommandHistory {
    history: Vec<HistoryEntry>,
    config: HistoryConfig,
}

impl CommandHistory {
    pub fn new() -> Result<Self> {
        let config = HistoryConfig::default();
        let history = Self::load_history()?;
        Ok(Self { history, config })
    }

    pub fn add_command(&mut self, command: String) -> Result<()> {
        if !self.config.save_history || command.trim().is_empty() {
            return Ok(());
        }

        // Skip commands with sensitive information
        if self.command_has_sensitive_info(&command) {
            return Ok(());
        }

        // Check for duplicate (don't add if it's the same as the last command)
        if let Some(last_entry) = self.history.last() {
            if last_entry.command == command {
                return Ok(());
            }
        }

        // Add new entry
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        
        self.history.push(HistoryEntry {
            command,
            timestamp,
        });

        // Trim history to max size
        if self.history.len() > self.config.max_size {
            self.history = self.history.split_off(self.history.len() - self.config.max_size);
        }

        self.save_history()?;
        Ok(())
    }

    pub fn get_commands(&self) -> Vec<String> {
        self.history.iter().map(|entry| entry.command.clone()).collect()
    }

    pub fn clear(&mut self) -> Result<()> {
        self.history.clear();
        self.save_history()?;
        Ok(())
    }

    fn load_history() -> Result<Vec<HistoryEntry>> {
        let history_path = Self::get_history_path()?;
        
        if !history_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(history_path)?;
        let history: Vec<HistoryEntry> = serde_json::from_str(&content)?;
        Ok(history)
    }

    fn save_history(&self) -> Result<()> {
        let history_path = Self::get_history_path()?;
        
        // Create directory if it doesn't exist
        if let Some(parent) = history_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.history)?;
        fs::write(history_path, content)?;
        Ok(())
    }

    fn get_history_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(HISTORY_FILE))
    }

    fn command_has_sensitive_info(&self, command: &str) -> bool {
        // Check built-in patterns
        for pattern in SENSITIVE_PATTERNS {
            if regex::Regex::new(pattern).unwrap().is_match(command) {
                return true;
            }
        }

        // Check additional patterns from config
        for pattern in &self.config.sensitive_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(command) {
                    return true;
                }
            }
        }

        false
    }
} 