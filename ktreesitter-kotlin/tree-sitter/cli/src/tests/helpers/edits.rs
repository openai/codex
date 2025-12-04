use std::{ops::Range, str};

#[derive(Debug)]
pub struct ReadRecorder<'a> {
    content: &'a [u8],
    indices_read: Vec<usize>,
}

impl<'a> ReadRecorder<'a> {
    #[must_use]
    pub const fn new(content: &'a [u8]) -> Self {
        Self {
            content,
            indices_read: Vec::new(),
        }
    }

    pub fn read(&mut self, offset: usize) -> &'a [u8] {
        if offset < self.content.len() {
            if let Err(i) = self.indices_read.binary_search(&offset) {
                self.indices_read.insert(i, offset);
            }
            &self.content[offset..(offset + 1)]
        } else {
            &[]
        }
    }

    pub fn strings_read(&self) -> Vec<&'a str> {
        let mut result = Vec::new();
        let mut last_range = Option::<Range<usize>>::None;
        for index in &self.indices_read {
            if let Some(ref mut range) = &mut last_range {
                if range.end == *index {
                    range.end += 1;
                } else {
                    result.push(str::from_utf8(&self.content[range.clone()]).unwrap());
                    last_range = None;
                }
            } else {
                last_range = Some(*index..(*index + 1));
            }
        }
        if let Some(range) = last_range {
            result.push(str::from_utf8(&self.content[range]).unwrap());
        }
        result
    }
}
