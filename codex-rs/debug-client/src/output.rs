#![allow(clippy::expect_used)]
use std::fs::File;
use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug)]
pub enum LabelColor {
    Assistant,
    Tool,
    ToolMeta,
    Thread,
}

#[derive(Debug, Default)]
struct PromptState {
    thread_id: Option<String>,
    visible: bool,
}

#[derive(Clone, Debug)]
pub struct Output {
    lock: Arc<Mutex<()>>,
    prompt: Arc<Mutex<PromptState>>,
    color: bool,
    jsonl_file: Option<Arc<Mutex<File>>>,
}

impl Output {
    pub fn new(jsonl_file: Option<File>) -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let color = !no_color && io::stdout().is_terminal() && io::stderr().is_terminal();
        Self {
            lock: Arc::new(Mutex::new(())),
            prompt: Arc::new(Mutex::new(PromptState::default())),
            color,
            jsonl_file: jsonl_file.map(|file| Arc::new(Mutex::new(file))),
        }
    }

    pub fn server_json_line(&self, line: &str, filtered_output: bool) -> io::Result<()> {
        let _guard = self.lock.lock().expect("output lock poisoned");

        if let Some(file) = self.jsonl_file.as_ref() {
            let mut file = file.lock().expect("jsonl file lock poisoned");
            writeln!(file, "{line}")?;
            file.flush()?;
        }

        if self.jsonl_file.is_none() && !filtered_output {
            self.clear_prompt_line_locked()?;
            let mut stdout = io::stdout();
            writeln!(stdout, "{line}")?;
            stdout.flush()?;
            self.redraw_prompt_locked()?;
        }

        Ok(())
    }

    pub fn server_line(&self, line: &str) -> io::Result<()> {
        let _guard = self.lock.lock().expect("output lock poisoned");
        self.clear_prompt_line_locked()?;
        let mut stdout = io::stdout();
        writeln!(stdout, "{line}")?;
        stdout.flush()?;
        self.redraw_prompt_locked()
    }

    pub fn client_line(&self, line: &str) -> io::Result<()> {
        let _guard = self.lock.lock().expect("output lock poisoned");
        self.clear_prompt_line_locked()?;
        let mut stderr = io::stderr();
        writeln!(stderr, "{line}")?;
        stderr.flush()
    }

    pub fn prompt(&self, thread_id: &str) -> io::Result<()> {
        let _guard = self.lock.lock().expect("output lock poisoned");
        self.set_prompt_locked(thread_id);
        self.write_prompt_locked()
    }

    pub fn set_prompt(&self, thread_id: &str) {
        let _guard = self.lock.lock().expect("output lock poisoned");
        self.set_prompt_locked(thread_id);
    }

    pub fn format_label(&self, label: &str, color: LabelColor) -> String {
        if !self.color {
            return label.to_string();
        }

        let code = match color {
            LabelColor::Assistant => "32",
            LabelColor::Tool => "36",
            LabelColor::ToolMeta => "33",
            LabelColor::Thread => "34",
        };
        format!("\x1b[{code}m{label}\x1b[0m")
    }

    fn clear_prompt_line_locked(&self) -> io::Result<()> {
        let mut prompt = self.prompt.lock().expect("prompt lock poisoned");
        if prompt.visible {
            let mut stderr = io::stderr();
            writeln!(stderr)?;
            stderr.flush()?;
            prompt.visible = false;
        }
        Ok(())
    }

    fn redraw_prompt_locked(&self) -> io::Result<()> {
        if self
            .prompt
            .lock()
            .expect("prompt lock poisoned")
            .thread_id
            .is_some()
        {
            self.write_prompt_locked()?;
        }
        Ok(())
    }

    fn set_prompt_locked(&self, thread_id: &str) {
        let mut prompt = self.prompt.lock().expect("prompt lock poisoned");
        prompt.thread_id = Some(thread_id.to_string());
    }

    fn write_prompt_locked(&self) -> io::Result<()> {
        let mut prompt = self.prompt.lock().expect("prompt lock poisoned");
        let Some(thread_id) = prompt.thread_id.as_ref() else {
            return Ok(());
        };
        let mut stderr = io::stderr();
        write!(stderr, "({thread_id})> ")?;
        stderr.flush()?;
        prompt.visible = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn server_json_line_writes_to_configured_file() {
        let path = std::env::temp_dir().join(format!(
            "codex-debug-client-output-{}.jsonl",
            std::process::id()
        ));
        let file = File::create(&path).expect("create output file");
        let output = Output::new(Some(file));

        output
            .server_json_line(r#"{"id":1}"#, false)
            .expect("write unfiltered line");
        output
            .server_json_line(r#"{"id":2}"#, true)
            .expect("write filtered line");

        assert_eq!(
            fs::read_to_string(&path).expect("read output file"),
            "{\"id\":1}\n{\"id\":2}\n"
        );
        let _ = fs::remove_file(path);
    }
}
