pub(super) type Replacement = (usize, usize, Vec<String>);

#[derive(Clone, Copy)]
enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
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
    /// Splits contents into logical lines while retaining each line ending.
    ///
    /// The first existing ending becomes the preferred style for inserted
    /// lines; files without an ending default to LF.
    pub(super) fn parse(contents: &str) -> Self {
        let mut lines = Vec::new();
        let mut preferred_ending = None;
        let mut line_start = 0;
        let mut cursor = 0;

        while cursor < contents.len() {
            let (ending, ending_len) = match contents.as_bytes()[cursor] {
                b'\r' if contents.as_bytes().get(cursor + 1) == Some(&b'\n') => {
                    (LineEnding::CrLf, 2)
                }
                b'\r' => (LineEnding::Cr, 1),
                b'\n' => (LineEnding::Lf, 1),
                _ => {
                    cursor += 1;
                    continue;
                }
            };
            preferred_ending.get_or_insert(ending);
            lines.push(SourceLine {
                text: contents[line_start..cursor].to_string(),
                ending: Some(ending),
            });
            cursor += ending_len;
            line_start = cursor;
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

    /// Rebuilds the file from source-ordered, non-overlapping replacements.
    ///
    /// Unchanged lines retain their original endings, inserted lines use the
    /// preferred ending, and every resulting line receives an ending to match
    /// apply-patch's historical trailing-newline behavior.
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
