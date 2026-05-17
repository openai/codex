use codex_utils_log::DEFAULT_BOUNDED_LOG_VALUE_BYTES;
use codex_utils_log::bounded_bytes_lossy;
use tracing::info;

pub(crate) struct StderrLogBuffer {
    program_name: String,
    buffer: Vec<u8>,
}

impl StderrLogBuffer {
    pub(crate) fn new(program_name: String) -> Self {
        Self {
            program_name,
            buffer: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            if let Some(newline_index) = bytes.iter().position(|byte| *byte == b'\n') {
                self.push_without_newline(&bytes[..newline_index]);
                self.log_complete_line();
                bytes = &bytes[newline_index + 1..];
            } else {
                self.push_without_newline(bytes);
                return;
            }
        }
    }

    pub(crate) fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        self.log_line("MCP server stderr");
        self.buffer.clear();
    }

    fn push_without_newline(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let remaining_capacity =
                DEFAULT_BOUNDED_LOG_VALUE_BYTES.saturating_sub(self.buffer.len());
            if remaining_capacity == 0 {
                self.log_line("MCP server stderr line exceeded limit");
                self.buffer.clear();
                continue;
            }

            let chunk_len = remaining_capacity.min(bytes.len());
            self.buffer.extend_from_slice(&bytes[..chunk_len]);
            bytes = &bytes[chunk_len..];

            if self.buffer.len() >= DEFAULT_BOUNDED_LOG_VALUE_BYTES {
                self.log_line("MCP server stderr line exceeded limit");
                self.buffer.clear();
            }
        }
    }

    fn log_complete_line(&mut self) {
        if self.buffer.last() == Some(&b'\r') {
            self.buffer.pop();
        }
        if self.buffer.is_empty() {
            return;
        }
        self.log_line("MCP server stderr");
        self.buffer.clear();
    }

    fn log_line(&self, label: &str) {
        info!(
            "{} ({}): {}",
            label,
            self.program_name,
            bounded_bytes_lossy(&self.buffer)
        );
    }

    #[cfg(test)]
    fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn push_without_newline_does_not_grow_past_log_limit() {
        let mut buffer = StderrLogBuffer::new("server".to_string());
        let bytes = vec![b'a'; DEFAULT_BOUNDED_LOG_VALUE_BYTES * 2 + 17];

        buffer.push(&bytes);

        assert_eq!(17, buffer.buffered_len());
    }

    #[test]
    fn push_newline_clears_buffer() {
        let mut buffer = StderrLogBuffer::new("server".to_string());

        buffer.push(b"hello\n");

        assert_eq!(0, buffer.buffered_len());
    }
}
