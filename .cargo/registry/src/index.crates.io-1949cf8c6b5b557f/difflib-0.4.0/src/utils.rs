pub fn calculate_ratio(matches: usize, length: usize) -> f32 {
    if length != 0 {
        return 2.0 * matches as f32 / length as f32;
    }
    1.0
}

pub fn str_with_similar_chars(c: char, length: usize) -> String {
    let mut s = String::new();
    for _ in 0..length {
        s.push_str(&c.to_string());
    }
    s
}

pub fn count_leading(line: &str, c: char) -> usize {
    let (mut i, n) = (0, line.len());
    let line: Vec<char> = line.chars().collect();
    while (i < n) && line[i] == c {
        i += 1;
    }
    i
}

pub fn format_range_unified(start: usize, end: usize) -> String {
    let mut beginning = start + 1;
    let length = end - start;
    if length == 1 {
        return beginning.to_string();
    }
    if length == 0 {
        beginning -= 1;
    }
    format!("{},{}", beginning, length)
}

pub fn format_range_context(start: usize, end: usize) -> String {
    let mut beginning = start + 1;
    let length = end - start;
    if length == 0 {
        beginning -= 1
    }
    if length <= 1 {
        return beginning.to_string();
    }
    format!("{},{}", beginning, beginning + length - 1)
}
