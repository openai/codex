#![deny(warnings)]
#[allow(unknown_lints)]
#[allow(unused_imports)]
use std::ascii::*;

pub struct CamelOptions {
    pub new_word: bool,
    pub last_char: char,
    pub first_word: bool,
    pub injectable_char: char,
    pub has_seperator: bool,
    pub inverted: bool,
}

pub fn to_case_snake_like(convertable_string: &str, replace_with: &str, case: &str) -> String {
    let mut first_character: bool = true;
    let mut result: String = String::with_capacity(convertable_string.len() * 2);
    for char_with_index in trim_right(convertable_string).char_indices() {
        if char_is_seperator(&char_with_index.1) {
            if !first_character {
                first_character = true;
                result.push(replace_with.chars().nth(0).unwrap_or('_'));
            }
        } else if requires_seperator(char_with_index, first_character, &convertable_string) {
            first_character = false;
            result = snake_like_with_seperator(result, replace_with, &char_with_index.1, case)
        } else {
            first_character = false;
            result = snake_like_no_seperator(result, &char_with_index.1, case)
        }
    }
    result
}

pub fn to_case_camel_like(convertable_string: &str, camel_options: CamelOptions) -> String {
    let mut new_word: bool = camel_options.new_word;
    let mut first_word: bool = camel_options.first_word;
    let mut last_char: char = camel_options.last_char;
    let mut found_real_char: bool = false;
    let mut result: String = String::with_capacity(convertable_string.len() * 2);
    for character in trim_right(convertable_string).chars() {
        if char_is_seperator(&character) && found_real_char {
            new_word = true;
        } else if !found_real_char && is_not_alphanumeric(character) {
            continue;
        } else if character.is_numeric() {
            found_real_char = true;
            new_word = true;
            result.push(character);
        } else if last_char_lower_current_is_upper_or_new_word(new_word, last_char, character) {
            found_real_char = true;
            new_word = false;
            result = append_on_new_word(result, first_word, character, &camel_options);
            first_word = false;
        } else {
            found_real_char = true;
            last_char = character;
            result.push(character.to_ascii_lowercase());
        }
    }
    result
}

#[inline]
fn append_on_new_word(mut result: String, first_word: bool, character: char, camel_options: &CamelOptions) -> String {
    if not_first_word_and_has_seperator(first_word, camel_options.has_seperator) {
        result.push(camel_options.injectable_char);
    }
    if first_word_or_not_inverted(first_word, camel_options.inverted) {
        result.push(character.to_ascii_uppercase());
    } else {
        result.push(character.to_ascii_lowercase());
    }
    result
}

fn not_first_word_and_has_seperator(first_word: bool, has_seperator: bool) -> bool {
    has_seperator && !first_word
}

fn first_word_or_not_inverted(first_word: bool, inverted: bool) -> bool {
    !inverted || first_word
}


fn last_char_lower_current_is_upper_or_new_word(new_word: bool, last_char: char, character: char) -> bool{
    new_word ||
        ((last_char.is_lowercase() && character.is_uppercase()) &&
         (last_char != ' '))
}

fn char_is_seperator(character: &char) -> bool {
    is_not_alphanumeric(*character)
}

fn trim_right(convertable_string: &str) -> &str {
    convertable_string.trim_end_matches(is_not_alphanumeric)
}

fn is_not_alphanumeric(character: char) -> bool {
    !character.is_alphanumeric()
}

#[inline]
fn requires_seperator(char_with_index: (usize, char), first_character: bool, convertable_string: &str) -> bool {
    !first_character &&
        char_is_uppercase(char_with_index.1) &&
            next_or_previous_char_is_lowercase(convertable_string, char_with_index.0)
}

#[inline]
fn snake_like_no_seperator(mut accumlator: String, current_char: &char, case: &str) -> String {
    if case == "lower" {
        accumlator.push(current_char.to_ascii_lowercase());
        accumlator
    } else {
        accumlator.push(current_char.to_ascii_uppercase());
        accumlator
    }
}

#[inline]
fn snake_like_with_seperator(mut accumlator: String, replace_with: &str, current_char: &char, case: &str) -> String {
    if case == "lower" {
        accumlator.push(replace_with.chars().nth(0).unwrap_or('_'));
        accumlator.push(current_char.to_ascii_lowercase());
        accumlator
    } else {
        accumlator.push(replace_with.chars().nth(0).unwrap_or('_'));
        accumlator.push(current_char.to_ascii_uppercase());
        accumlator
    }
}

fn next_or_previous_char_is_lowercase(convertable_string: &str, char_with_index: usize) -> bool {
    convertable_string.chars().nth(char_with_index + 1).unwrap_or('A').is_lowercase() ||
        convertable_string.chars().nth(char_with_index - 1).unwrap_or('A').is_lowercase()
}

fn char_is_uppercase(test_char: char) -> bool {
    test_char == test_char.to_ascii_uppercase()
}

#[test]
fn test_trim_bad_chars() {
    assert_eq!("abc", trim_right("abc----^"))
}

#[test]
fn test_trim_bad_chars_when_none_are_bad() {
    assert_eq!("abc", trim_right("abc"))
}

#[test]
fn test_is_not_alphanumeric_on_is_alphanumeric() {
    assert!(!is_not_alphanumeric('a'))
}

#[test]
fn test_is_not_alphanumeric_on_is_not_alphanumeric() {
    assert!(is_not_alphanumeric('_'))
}


#[test]
fn test_char_is_uppercase_when_it_is() {
    assert_eq!(char_is_uppercase('A'), true)
}

#[test]
fn test_char_is_uppercase_when_it_is_not() {
    assert_eq!(char_is_uppercase('a'), false)
}

#[test]
fn test_next_or_previous_char_is_lowercase_true() {
    assert_eq!(next_or_previous_char_is_lowercase("TestWWW", 3), true)
}

#[test]
fn test_next_or_previous_char_is_lowercase_false() {
    assert_eq!(next_or_previous_char_is_lowercase("TestWWW", 5), false)
}

#[test]
fn snake_like_with_seperator_lowers() {
    assert_eq!(snake_like_with_seperator("".to_owned(), "^", &'c', "lower"), "^c".to_string())
}

#[test]
fn snake_like_with_seperator_upper() {
    assert_eq!(snake_like_with_seperator("".to_owned(), "^", &'c', "upper"), "^C".to_string())
}

#[test]
fn snake_like_no_seperator_lower() {
    assert_eq!(snake_like_no_seperator("".to_owned(), &'C', "lower"), "c".to_string())
}

#[test]
fn snake_like_no_seperator_upper() {
    assert_eq!(snake_like_no_seperator("".to_owned(), &'c', "upper"), "C".to_string())
}

#[test]
fn requires_seperator_upper_not_first_wrap_is_safe_current_upper() {
    assert_eq!(requires_seperator((2, 'C'), false, "test"), true)
}

#[test]
fn requires_seperator_upper_not_first_wrap_is_safe_current_lower() {
    assert_eq!(requires_seperator((2, 'c'), false, "test"), false)
}

#[test]
fn requires_seperator_upper_first_wrap_is_safe_current_upper() {
    assert_eq!(requires_seperator((0, 'T'), true, "Test"), false)
}

#[test]
fn requires_seperator_upper_first_wrap_is_safe_current_lower() {
    assert_eq!(requires_seperator((0, 't'), true, "Test"), false)
}

#[test]
fn requires_seperator_upper_first_wrap_is_safe_current_lower_next_is_too() {
    assert_eq!(requires_seperator((0, 't'), true, "test"), false)
}

#[test]
fn test_char_is_seperator_dash() {
    assert_eq!(char_is_seperator(&'-'), true)
}

#[test]
fn test_char_is_seperator_underscore() {
    assert_eq!(char_is_seperator(&'_'), true)
}

#[test]
fn test_char_is_seperator_space() {
    assert_eq!(char_is_seperator(&' '), true)
}

#[test]
fn test_char_is_seperator_when_not() {
    assert_eq!(char_is_seperator(&'A'), false)
}

#[test]
fn test_last_char_lower_current_is_upper_or_new_word_with_new_word() {
    assert_eq!(last_char_lower_current_is_upper_or_new_word(true, ' ', '-'), true)
}

#[test]
fn test_last_char_lower_current_is_upper_or_new_word_last_char_space() {
    assert_eq!(last_char_lower_current_is_upper_or_new_word(false, ' ', '-'), false)
}

#[test]
fn test_last_char_lower_current_is_upper_or_new_word_last_char_lower_current_upper() {
    assert_eq!(last_char_lower_current_is_upper_or_new_word(false, 'a', 'A'), true)
}

#[test]
fn test_last_char_lower_current_is_upper_or_new_word_last_char_upper_current_upper() {
    assert_eq!(last_char_lower_current_is_upper_or_new_word(false, 'A', 'A'), false)
}

#[test]
fn test_last_char_lower_current_is_upper_or_new_word_last_char_upper_current_lower() {
    assert_eq!(last_char_lower_current_is_upper_or_new_word(false, 'A', 'a'), false)
}

#[test]
fn test_first_word_or_not_inverted_with_first_word() {
    assert_eq!(first_word_or_not_inverted(true, false), true)
}

#[test]
fn test_first_word_or_not_inverted_not_first_word_not_inverted() {
    assert_eq!(first_word_or_not_inverted(false, false), true)
}

#[test]
fn test_first_word_or_not_inverted_not_first_word_is_inverted() {
    assert_eq!(first_word_or_not_inverted(false, true), false)
}

#[test]
fn test_not_first_word_and_has_seperator_is_first_and_not_seperator() {
    assert_eq!(not_first_word_and_has_seperator(true, false), false)
}

#[test]
fn test_not_first_word_and_has_seperator_not_first_and_not_seperator() {
    assert_eq!(not_first_word_and_has_seperator(false, false), false)
}

#[test]
fn test_not_first_word_and_has_seperator_not_first_and_has_seperator() {
    assert_eq!(not_first_word_and_has_seperator(false, true), true)
}
