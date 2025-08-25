use super::config_types::{OutputConfig, OutputTruncateStrategy, OutputVerbosity};
use std::collections::VecDeque;

pub struct OutputFormatter {
    pub config: OutputConfig,
    last_output: Option<String>,
}

impl OutputFormatter {
    pub fn new(config: OutputConfig) -> Self {
        Self {
            config,
            last_output: None,
        }
    }

    pub fn format_command_output(&mut self, output: &str, command: &str) -> String {
        self.last_output = Some(output.to_string());
        
        match self.config.verbosity {
            OutputVerbosity::Full => output.to_string(),
            OutputVerbosity::Verbose => self.format_verbose(output),
            OutputVerbosity::Auto => self.format_auto(output, command),
            OutputVerbosity::Summary => self.format_summary(output),
        }
    }

    pub fn get_last_full_output(&self) -> Option<String> {
        self.last_output.clone()
    }

    fn format_verbose(&self, output: &str) -> String {
        let lines: Vec<&str> = output.lines().collect();
        
        if lines.len() <= self.config.max_lines {
            return output.to_string();
        }
        
        self.truncate_output(&lines)
    }

    fn format_auto(&self, output: &str, command: &str) -> String {
        if self.config.auto_expand_errors && self.has_error(output) {
            return self.format_verbose(output);
        }
        
        if self.is_important_command(command) {
            return self.format_verbose(output);
        }
        
        self.format_summary(output)
    }

    fn format_summary(&self, output: &str) -> String {
        let lines: Vec<&str> = output.lines().collect();
        let total_lines = lines.len();
        
        if total_lines <= 10 {
            return output.to_string();
        }
        
        let summary_lines = 5;
        let head: Vec<String> = lines.iter()
            .take(summary_lines)
            .map(|&s| s.to_string())
            .collect();
        
        let tail: Vec<String> = lines.iter()
            .rev()
            .take(summary_lines)
            .rev()
            .map(|&s| s.to_string())
            .collect();
        
        format!(
            "{}\n... ({} lines omitted) ...\n{}",
            head.join("\n"),
            total_lines - (summary_lines * 2),
            tail.join("\n")
        )
    }

    fn truncate_output(&self, lines: &[&str]) -> String {
        let max_lines = self.config.max_lines;
        let total_lines = lines.len();
        
        match self.config.truncate_strategy {
            OutputTruncateStrategy::Head => {
                let truncated: Vec<String> = lines.iter()
                    .take(max_lines)
                    .map(|&s| s.to_string())
                    .collect();
                format!(
                    "{}\n... ({} more lines)",
                    truncated.join("\n"),
                    total_lines - max_lines
                )
            }
            OutputTruncateStrategy::Tail => {
                let truncated: Vec<String> = lines.iter()
                    .rev()
                    .take(max_lines)
                    .rev()
                    .map(|&s| s.to_string())
                    .collect();
                format!(
                    "... ({} previous lines)\n{}",
                    total_lines - max_lines,
                    truncated.join("\n")
                )
            }
            OutputTruncateStrategy::Middle => {
                let head_lines = max_lines / 2;
                let tail_lines = max_lines - head_lines;
                
                let head: Vec<String> = lines.iter()
                    .take(head_lines)
                    .map(|&s| s.to_string())
                    .collect();
                
                let tail: Vec<String> = lines.iter()
                    .rev()
                    .take(tail_lines)
                    .rev()
                    .map(|&s| s.to_string())
                    .collect();
                
                format!(
                    "{}\n... ({} lines omitted) ...\n{}",
                    head.join("\n"),
                    total_lines - max_lines,
                    tail.join("\n")
                )
            }
        }
    }

    fn has_error(&self, output: &str) -> bool {
        let error_indicators = [
            "error:",
            "Error:",
            "ERROR:",
            "failed",
            "Failed",
            "FAILED",
            "panic:",
            "Panic:",
            "PANIC:",
            "fatal:",
            "Fatal:",
            "FATAL:",
            "exception:",
            "Exception:",
            "EXCEPTION:",
            "warning:",
            "Warning:",
            "WARNING:",
        ];
        
        let lower_output = output.to_lowercase();
        error_indicators.iter().any(|&indicator| lower_output.contains(&indicator.to_lowercase()))
    }

    fn is_important_command(&self, command: &str) -> bool {
        let important_patterns = [
            "test",
            "build",
            "compile",
            "npm run",
            "cargo",
            "make",
            "pytest",
            "jest",
            "mocha",
            "rspec",
            "go test",
            "mvn",
            "gradle",
            "yarn",
            "pnpm",
            "docker",
            "kubectl",
        ];
        
        let lower_command = command.to_lowercase();
        important_patterns.iter().any(|&pattern| lower_command.contains(pattern))
    }
}

pub struct OutputBuffer {
    buffer: VecDeque<(String, String)>,
    max_entries: usize,
}

impl OutputBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }

    pub fn add(&mut self, command: String, output: String) {
        if self.buffer.len() >= self.max_entries {
            self.buffer.pop_front();
        }
        self.buffer.push_back((command, output));
    }

    pub fn get_last(&self) -> Option<&(String, String)> {
        self.buffer.back()
    }

    pub fn get_by_index(&self, index: usize) -> Option<&(String, String)> {
        if index < self.buffer.len() {
            self.buffer.get(self.buffer.len() - 1 - index)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_summary() {
        let config = OutputConfig {
            verbosity: OutputVerbosity::Summary,
            max_lines: 100,
            truncate_strategy: OutputTruncateStrategy::Middle,
            auto_expand_errors: false,
        };
        
        let mut formatter = OutputFormatter::new(config);
        
        let short_output = "Line 1\nLine 2\nLine 3";
        assert_eq!(
            formatter.format_command_output(short_output, "echo test"),
            short_output
        );
        
        let long_output = (1..=100).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        let formatted = formatter.format_command_output(&long_output, "echo test");
        assert!(formatted.contains("... (90 lines omitted) ..."));
    }

    #[test]
    fn test_error_detection() {
        let config = OutputConfig {
            verbosity: OutputVerbosity::Auto,
            max_lines: 100,
            truncate_strategy: OutputTruncateStrategy::Middle,
            auto_expand_errors: true,
        };
        
        let mut formatter = OutputFormatter::new(config);
        
        let error_output = "Starting build...\nError: compilation failed\nSee logs for details";
        let formatted = formatter.format_command_output(error_output, "build");
        assert_eq!(formatted, error_output);
    }

    #[test]
    fn test_important_command_detection() {
        let config = OutputConfig {
            verbosity: OutputVerbosity::Auto,
            max_lines: 100,
            truncate_strategy: OutputTruncateStrategy::Middle,
            auto_expand_errors: false,
        };
        
        let mut formatter = OutputFormatter::new(config);
        
        let build_output = "Building project...\nCompiling...\nDone!";
        let formatted = formatter.format_command_output(build_output, "npm run build");
        assert_eq!(formatted, build_output);
    }

    #[test]
    fn test_truncate_strategies() {
        let config_head = OutputConfig {
            verbosity: OutputVerbosity::Verbose,
            max_lines: 5,
            truncate_strategy: OutputTruncateStrategy::Head,
            auto_expand_errors: false,
        };
        
        let mut formatter = OutputFormatter::new(config_head);
        let long_output = (1..=20).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        
        let formatted = formatter.format_command_output(&long_output, "test");
        assert!(formatted.contains("Line 1"));
        assert!(formatted.contains("Line 5"));
        assert!(!formatted.contains("Line 20"));
        assert!(formatted.contains("... (15 more lines)"));
        
        let config_tail = OutputConfig {
            verbosity: OutputVerbosity::Verbose,
            max_lines: 5,
            truncate_strategy: OutputTruncateStrategy::Tail,
            auto_expand_errors: false,
        };
        
        let mut formatter = OutputFormatter::new(config_tail);
        let formatted = formatter.format_command_output(&long_output, "test");
        assert!(!formatted.contains("Line 15"));
        assert!(formatted.contains("Line 16"));
        assert!(formatted.contains("Line 20"));
        assert!(formatted.contains("... (15 previous lines)"));
    }

    #[test]
    fn test_output_buffer() {
        let mut buffer = OutputBuffer::new(3);
        
        buffer.add("cmd1".to_string(), "output1".to_string());
        buffer.add("cmd2".to_string(), "output2".to_string());
        buffer.add("cmd3".to_string(), "output3".to_string());
        
        assert_eq!(buffer.get_last(), Some(&("cmd3".to_string(), "output3".to_string())));
        
        buffer.add("cmd4".to_string(), "output4".to_string());
        assert_eq!(buffer.buffer.len(), 3);
        assert_eq!(buffer.get_last(), Some(&("cmd4".to_string(), "output4".to_string())));
        assert_eq!(buffer.get_by_index(0), Some(&("cmd4".to_string(), "output4".to_string())));
        assert_eq!(buffer.get_by_index(1), Some(&("cmd3".to_string(), "output3".to_string())));
    }
}