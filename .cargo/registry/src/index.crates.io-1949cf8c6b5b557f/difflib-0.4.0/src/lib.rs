pub mod differ;
pub mod sequencematcher;
mod utils;

use sequencematcher::{Sequence, SequenceMatcher};
use std::collections::HashMap;
use std::fmt::Display;
use utils::{format_range_context, format_range_unified};

pub fn get_close_matches<'a>(
    word: &str,
    possibilities: Vec<&'a str>,
    n: usize,
    cutoff: f32,
) -> Vec<&'a str> {
    if !(0.0 <= cutoff && cutoff <= 1.0) {
        panic!("Cutoff must be greater than 0.0 and lower than 1.0");
    }
    let mut res: Vec<(f32, &str)> = Vec::new();
    let mut matcher = SequenceMatcher::new("", word);
    for i in &possibilities {
        matcher.set_first_seq(i);
        let ratio = matcher.ratio();
        if ratio >= cutoff {
            res.push((ratio, i));
        }
    }
    res.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    res.truncate(n);
    res.iter().map(|x| x.1).collect()
}

pub fn unified_diff<T: Sequence + Display>(
    first_sequence: &[T],
    second_sequence: &[T],
    from_file: &str,
    to_file: &str,
    from_file_date: &str,
    to_file_date: &str,
    n: usize,
) -> Vec<String> {
    let mut res = Vec::new();
    let lineterm = '\n';
    let mut started = false;
    let mut matcher = SequenceMatcher::new(first_sequence, second_sequence);
    for group in &matcher.get_grouped_opcodes(n) {
        if !started {
            started = true;
            let from_date = format!("\t{}", from_file_date);
            let to_date = format!("\t{}", to_file_date);
            res.push(format!("--- {}{}{}", from_file, from_date, lineterm));
            res.push(format!("+++ {}{}{}", to_file, to_date, lineterm));
        }
        let (first, last) = (group.first().unwrap(), group.last().unwrap());
        let file1_range = format_range_unified(first.first_start, last.first_end);
        let file2_range = format_range_unified(first.second_start, last.second_end);
        res.push(format!(
            "@@ -{} +{} @@{}",
            file1_range, file2_range, lineterm
        ));
        for code in group {
            if code.tag == "equal" {
                for item in first_sequence
                    .iter()
                    .take(code.first_end)
                    .skip(code.first_start)
                {
                    res.push(format!(" {}", item));
                }
                continue;
            }
            if code.tag == "replace" || code.tag == "delete" {
                for item in first_sequence
                    .iter()
                    .take(code.first_end)
                    .skip(code.first_start)
                {
                    res.push(format!("-{}", item));
                }
            }
            if code.tag == "replace" || code.tag == "insert" {
                for item in second_sequence
                    .iter()
                    .take(code.second_end)
                    .skip(code.second_start)
                {
                    res.push(format!("+{}", item));
                }
            }
        }
    }
    res
}

pub fn context_diff<T: Sequence + Display>(
    first_sequence: &[T],
    second_sequence: &[T],
    from_file: &str,
    to_file: &str,
    from_file_date: &str,
    to_file_date: &str,
    n: usize,
) -> Vec<String> {
    let mut res = Vec::new();
    let lineterm = '\n';
    let mut prefix: HashMap<String, String> = HashMap::new();
    prefix.insert(String::from("insert"), String::from("+ "));
    prefix.insert(String::from("delete"), String::from("- "));
    prefix.insert(String::from("replace"), String::from("! "));
    prefix.insert(String::from("equal"), String::from("  "));
    let mut started = false;
    let mut matcher = SequenceMatcher::new(first_sequence, second_sequence);
    for group in &matcher.get_grouped_opcodes(n) {
        if !started {
            started = true;
            let from_date = format!("\t{}", from_file_date);
            let to_date = format!("\t{}", to_file_date);
            res.push(format!("*** {}{}{}", from_file, from_date, lineterm));
            res.push(format!("--- {}{}{}", to_file, to_date, lineterm));
        }
        let (first, last) = (group.first().unwrap(), group.last().unwrap());
        res.push(format!("***************{}", lineterm));
        let file1_range = format_range_context(first.first_start, last.first_end);
        res.push(format!("*** {} ****{}", file1_range, lineterm));
        let mut any = false;
        for opcode in group {
            if opcode.tag == "replace" || opcode.tag == "delete" {
                any = true;
                break;
            }
        }
        if any {
            for opcode in group {
                if opcode.tag != "insert" {
                    for item in first_sequence
                        .iter()
                        .take(opcode.first_end)
                        .skip(opcode.first_start)
                    {
                        res.push(format!("{}{}", &prefix[&opcode.tag], item));
                    }
                }
            }
        }
        let file2_range = format_range_context(first.second_start, last.second_end);
        res.push(format!("--- {} ----{}", file2_range, lineterm));
        any = false;
        for opcode in group {
            if opcode.tag == "replace" || opcode.tag == "insert" {
                any = true;
                break;
            }
        }
        if any {
            for opcode in group {
                if opcode.tag != "delete" {
                    for item in second_sequence
                        .iter()
                        .take(opcode.second_end)
                        .skip(opcode.second_start)
                    {
                        res.push(format!("{}{}", prefix[&opcode.tag], item));
                    }
                }
            }
        }
    }
    res
}
