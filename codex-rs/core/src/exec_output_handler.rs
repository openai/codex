use super::config::Config;
use super::output_formatter::{OutputBuffer, OutputFormatter};
use std::sync::{Arc, Mutex};

pub struct ExecOutputHandler {
    formatter: Arc<Mutex<OutputFormatter>>,
    buffer: Arc<Mutex<OutputBuffer>>,
}

impl ExecOutputHandler {
    pub fn new(config: &Config) -> Self {
        let formatter = OutputFormatter::new(config.output.clone());
        let buffer = OutputBuffer::new(100);
        
        Self {
            formatter: Arc::new(Mutex::new(formatter)),
            buffer: Arc::new(Mutex::new(buffer)),
        }
    }

    pub fn format_output(&self, output: &str, command: &str) -> String {
        let mut formatter = self.formatter.lock().unwrap();
        let formatted = formatter.format_command_output(output, command);
        
        let mut buffer = self.buffer.lock().unwrap();
        buffer.add(command.to_string(), output.to_string());
        
        formatted
    }

    pub fn get_last_full_output(&self) -> Option<String> {
        let buffer = self.buffer.lock().unwrap();
        buffer.get_last().map(|(_, output)| output.clone())
    }

    pub fn set_verbosity(&self, verbosity: super::config_types::OutputVerbosity) {
        let mut formatter = self.formatter.lock().unwrap();
        let mut config = formatter.config.clone();
        config.verbosity = verbosity;
        *formatter = OutputFormatter::new(config);
    }

    pub fn get_output_by_index(&self, index: usize) -> Option<(String, String)> {
        let buffer = self.buffer.lock().unwrap();
        buffer.get_by_index(index).cloned()
    }
}