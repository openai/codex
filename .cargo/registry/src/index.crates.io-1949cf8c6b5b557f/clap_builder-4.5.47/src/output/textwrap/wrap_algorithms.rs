use super::core::display_width;

#[derive(Debug)]
pub(crate) struct LineWrapper<'w> {
    hard_width: usize,
    line_width: usize,
    carryover: Option<&'w str>,
}

impl<'w> LineWrapper<'w> {
    pub(crate) fn new(hard_width: usize) -> Self {
        Self {
            hard_width,
            line_width: 0,
            carryover: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.line_width = 0;
        self.carryover = None;
    }

    pub(crate) fn wrap(&mut self, mut words: Vec<&'w str>) -> Vec<&'w str> {
        if self.carryover.is_none() {
            if let Some(word) = words.first() {
                if word.trim().is_empty() {
                    self.carryover = Some(*word);
                } else {
                    self.carryover = Some("");
                }
            }
        }

        let mut i = 0;
        while i < words.len() {
            let word = &words[i];
            let trimmed = word.trim_end();
            let word_width = display_width(trimmed);
            let trimmed_delta = word.len() - trimmed.len();
            if i != 0 && self.hard_width < self.line_width + word_width {
                if 0 < i {
                    let last = i - 1;
                    let trimmed = words[last].trim_end();
                    words[last] = trimmed;
                }

                self.line_width = 0;
                words.insert(i, "\n");
                i += 1;
                if let Some(carryover) = self.carryover {
                    words.insert(i, carryover);
                    self.line_width += carryover.len();
                    i += 1;
                }
            }
            self.line_width += word_width + trimmed_delta;

            i += 1;
        }
        words
    }
}
