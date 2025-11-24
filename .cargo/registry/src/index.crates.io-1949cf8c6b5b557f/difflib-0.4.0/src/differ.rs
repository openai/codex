use sequencematcher::SequenceMatcher;
use std::cmp;
use utils::{count_leading, str_with_similar_chars};

#[derive(Default)]
pub struct Differ {
    pub line_junk: Option<fn(&&str) -> bool>,
    pub char_junk: Option<fn(&char) -> bool>,
}

impl Differ {
    pub fn new() -> Differ {
        Differ {
            line_junk: None,
            char_junk: None,
        }
    }

    pub fn compare(&self, first_sequence: &[&str], second_sequence: &[&str]) -> Vec<String> {
        let mut matcher = SequenceMatcher::new(first_sequence, second_sequence);
        matcher.set_is_junk(self.line_junk);
        let mut res = Vec::new();
        for opcode in matcher.get_opcodes() {
            let mut gen = Vec::new();
            match opcode.tag.as_ref() {
                "replace" => {
                    gen = self.fancy_replace(
                        first_sequence,
                        opcode.first_start,
                        opcode.first_end,
                        second_sequence,
                        opcode.second_start,
                        opcode.second_end,
                    )
                }
                "delete" => {
                    gen = self.dump("-", first_sequence, opcode.first_start, opcode.first_end)
                }
                "insert" => {
                    gen = self.dump("+", second_sequence, opcode.second_start, opcode.second_end)
                }
                "equal" => {
                    gen = self.dump(" ", first_sequence, opcode.first_start, opcode.first_end)
                }
                _ => {}
            }
            for i in gen {
                res.push(i);
            }
        }
        res
    }

    fn dump(&self, tag: &str, sequence: &[&str], start: usize, end: usize) -> Vec<String> {
        let mut res = Vec::new();
        for i in start..end {
            if let Some(s) = sequence.get(i) {
                res.push(format!("{} {}", tag, s))
            }
        }
        res
    }

    fn plain_replace(
        &self,
        first_sequence: &[&str],
        first_start: usize,
        first_end: usize,
        second_sequence: &[&str],
        second_start: usize,
        second_end: usize,
    ) -> Vec<String> {
        if !(first_start < first_end && second_start < second_end) {
            return Vec::new();
        }
        let (mut first, second) = if second_end - second_start < first_end - first_start {
            (
                self.dump("+", second_sequence, second_start, second_end),
                self.dump("-", first_sequence, first_start, first_end),
            )
        } else {
            (
                self.dump("-", first_sequence, first_start, first_end),
                self.dump("+", second_sequence, second_start, second_end),
            )
        };
        for s in second {
            first.push(s);
        }
        first
    }

    fn fancy_replace(
        &self,
        first_sequence: &[&str],
        first_start: usize,
        first_end: usize,
        second_sequence: &[&str],
        second_start: usize,
        second_end: usize,
    ) -> Vec<String> {
        let mut res = Vec::new();
        let (mut best_ratio, cutoff) = (0.74, 0.75);
        let (mut best_i, mut best_j) = (0, 0);
        let mut eqi: Option<usize> = None;
        let mut eqj: Option<usize> = None;
        for (j, second_sequence_str) in second_sequence
            .iter()
            .enumerate()
            .take(second_end)
            .skip(second_start)
        {
            for (i, first_sequence_str) in first_sequence
                .iter()
                .enumerate()
                .take(second_end)
                .skip(second_start)
            {
                if first_sequence_str == second_sequence_str {
                    if eqi.is_none() {
                        eqi = Some(i);
                        eqj = Some(j);
                    }
                    continue;
                }
                let (first_sequence_chars, second_sequence_chars) = (
                    first_sequence_str.chars().collect::<Vec<char>>(),
                    second_sequence_str.chars().collect::<Vec<char>>(),
                );
                let mut cruncher =
                    SequenceMatcher::new(&first_sequence_chars, &second_sequence_chars);
                cruncher.set_is_junk(self.char_junk);
                if cruncher.ratio() > best_ratio {
                    best_ratio = cruncher.ratio();
                    best_i = i;
                    best_j = j;
                }
            }
        }
        if best_ratio < cutoff {
            if eqi.is_none() {
                res.extend(
                    self.plain_replace(
                        first_sequence,
                        first_start,
                        first_end,
                        second_sequence,
                        second_start,
                        second_end,
                    ).iter()
                        .cloned(),
                );
                return res;
            }
            best_i = eqi.unwrap();
            best_j = eqj.unwrap();
        } else {
            eqi = None;
        }
        res.extend(
            self.fancy_helper(
                first_sequence,
                first_start,
                best_i,
                second_sequence,
                second_start,
                best_j,
            ).iter()
                .cloned(),
        );
        let first_element = &first_sequence[best_i];
        let second_element = &second_sequence[best_j];
        if eqi.is_none() {
            let (mut first_tag, mut second_tag) = (String::new(), String::new());
            let first_element_chars: Vec<char> = first_element.chars().collect();
            let second_element_chars: Vec<char> = second_element.chars().collect();
            let mut cruncher = SequenceMatcher::new(&first_element_chars, &second_element_chars);
            cruncher.set_is_junk(self.char_junk);
            for opcode in &cruncher.get_opcodes() {
                let (first_length, second_length) = (
                    opcode.first_end - opcode.first_start,
                    opcode.second_end - opcode.second_start,
                );
                match opcode.tag.as_ref() {
                    "replace" => {
                        first_tag.push_str(&str_with_similar_chars('^', first_length));
                        second_tag.push_str(&str_with_similar_chars('^', second_length));
                    }
                    "delete" => {
                        first_tag.push_str(&str_with_similar_chars('-', first_length));
                    }
                    "insert" => {
                        second_tag.push_str(&str_with_similar_chars('+', second_length));
                    }
                    "equal" => {
                        first_tag.push_str(&str_with_similar_chars(' ', first_length));
                        second_tag.push_str(&str_with_similar_chars(' ', second_length));
                    }
                    _ => {}
                }
            }
            res.extend(
                self.qformat(&first_element, &second_element, &first_tag, &second_tag)
                    .iter()
                    .cloned(),
            );
        } else {
            let mut s = String::from("  ");
            s.push_str(&first_element);
            res.push(s);
        }
        res.extend(
            self.fancy_helper(
                first_sequence,
                best_i + 1,
                first_end,
                second_sequence,
                best_j + 1,
                second_end,
            ).iter()
                .cloned(),
        );
        res
    }

    fn fancy_helper(
        &self,
        first_sequence: &[&str],
        first_start: usize,
        first_end: usize,
        second_sequence: &[&str],
        second_start: usize,
        second_end: usize,
    ) -> Vec<String> {
        let mut res = Vec::new();
        if first_start < first_end {
            if second_start < second_end {
                res = self.fancy_replace(
                    first_sequence,
                    first_start,
                    first_end,
                    second_sequence,
                    second_start,
                    second_end,
                );
            } else {
                res = self.dump("-", first_sequence, first_start, first_end);
            }
        } else if second_start < second_end {
            res = self.dump("+", second_sequence, second_start, second_end);
        }
        res
    }

    fn qformat(
        &self,
        first_line: &str,
        second_line: &str,
        first_tags: &str,
        second_tags: &str,
    ) -> Vec<String> {
        let mut res = Vec::new();
        let mut first_tags = first_tags;
        let mut second_tags = second_tags;
        let mut common = cmp::min(
            count_leading(first_line, '\t'),
            count_leading(second_line, '\t'),
        );
        common = cmp::min(common, count_leading(first_tags.split_at(common).0, ' '));
        common = cmp::min(common, count_leading(first_tags.split_at(common).0, ' '));
        first_tags = first_tags.split_at(common).1.trim_right();
        second_tags = second_tags.split_at(common).1.trim_right();
        let mut s = format!("- {}", first_line);
        res.push(s);
        if first_tags != "" {
            s = format!("? {}{}\n", str_with_similar_chars('\t', common), first_tags);
            res.push(s);
        }
        s = format!("+ {}", second_line);
        res.push(s);
        if second_tags != "" {
            s = format!(
                "? {}{}\n",
                str_with_similar_chars('\t', common),
                second_tags
            );
            res.push(s);
        }
        res
    }

    pub fn restore(delta: &[String], which: usize) -> Vec<String> {
        if !(which == 1 || which == 2) {
            panic!("Second parameter must be 1 or 2");
        }
        let mut res = Vec::new();
        let tag = if which == 1 { "- " } else { "+ " }.to_string();
        let prefixes = vec![tag, "  ".to_string()];
        for line in delta {
            for prefix in &prefixes {
                if line.starts_with(prefix) {
                    res.push(line.split_at(2).1.to_string());
                }
            }
        }
        res
    }
}

#[test]
fn test_fancy_replace() {
    let differ = Differ::new();
    let result = differ
        .fancy_replace(&vec!["abcDefghiJkl\n"], 0, 1, &vec!["abcdefGhijkl\n"], 0, 1)
        .join("");
    assert_eq!(
        result,
        "- abcDefghiJkl\n?    ^  ^  ^\n+ abcdefGhijkl\n?    ^  ^  ^\n"
    );
}

#[test]
fn test_qformat() {
    let differ = Differ::new();
    let result = differ.qformat(
        "\tabcDefghiJkl\n",
        "\tabcdefGhijkl\n",
        "  ^ ^  ^      ",
        "  ^ ^  ^      ",
    );
    assert_eq!(
        result,
        vec![
            "- \tabcDefghiJkl\n",
            "? \t ^ ^  ^\n",
            "+ \tabcdefGhijkl\n",
            "? \t ^ ^  ^\n",
        ]
    );
}
