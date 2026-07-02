pub(super) type Replacement = (usize, usize, Vec<String>);

#[derive(Clone, Copy)]
enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

struct SourceLine {
    text: String,
    ending: Option<LineEnding>,
}

pub(super) struct SourceFile {
    lines: Vec<SourceLine>,
    preferred_ending: LineEnding,
}

impl SourceFile {
    pub(super) fn parse(contents: &str) -> Self {
        let mut lines = Vec::new();
        let mut preferred_ending = None;
        let mut line_start = 0;

        for (newline, _) in contents.match_indices('\n') {
            let line = &contents[line_start..newline];
            let (text, ending) = if let Some(text) = line.strip_suffix('\r') {
                (text, LineEnding::CrLf)
            } else {
                (line, LineEnding::Lf)
            };
            // Match rustfmt and Ruff's auto-detection behavior by using the
            // first existing newline as the file's preferred style.
            preferred_ending.get_or_insert(ending);
            lines.push(SourceLine {
                text: text.to_string(),
                ending: Some(ending),
            });
            line_start = newline + 1;
        }

        if line_start < contents.len() {
            lines.push(SourceLine {
                text: contents[line_start..].to_string(),
                ending: None,
            });
        }

        Self {
            lines,
            preferred_ending: preferred_ending.unwrap_or(LineEnding::Lf),
        }
    }

    pub(super) fn line_texts(&self) -> Vec<String> {
        self.lines.iter().map(|line| line.text.clone()).collect()
    }

    pub(super) fn apply_replacements(&mut self, replacements: &[Replacement]) {
        let mut source_lines = std::mem::take(&mut self.lines).into_iter();
        let mut new_lines = Vec::new();
        let mut source_index = 0;

        for (start_idx, old_len, new_segment) in replacements {
            debug_assert!(*start_idx >= source_index);
            for line in source_lines.by_ref().take(*start_idx - source_index) {
                new_lines.push(line);
            }
            for _ in source_lines.by_ref().take(*old_len) {}
            new_lines.extend(new_segment.iter().map(|text| SourceLine {
                text: text.clone(),
                ending: Some(self.preferred_ending),
            }));
            source_index = start_idx + old_len;
        }
        new_lines.extend(source_lines);
        self.lines = new_lines;

        // Updates have historically added a trailing newline. This also gives
        // an unterminated last line an ending if an insertion moved it inward.
        for line in &mut self.lines {
            line.ending.get_or_insert(self.preferred_ending);
        }
    }

    pub(super) fn into_contents(self) -> String {
        let mut contents = String::new();
        for line in self.lines {
            contents.push_str(&line.text);
            if let Some(ending) = line.ending {
                contents.push_str(ending.as_str());
            }
        }
        contents
    }
}
