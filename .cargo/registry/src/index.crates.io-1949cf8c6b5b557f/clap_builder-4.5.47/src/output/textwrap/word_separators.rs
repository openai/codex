pub(crate) fn find_words_ascii_space(line: &str) -> impl Iterator<Item = &'_ str> + '_ {
    let mut start = 0;
    let mut in_whitespace = false;
    let mut char_indices = line.char_indices();

    std::iter::from_fn(move || {
        for (idx, ch) in char_indices.by_ref() {
            let next_whitespace = ch == ' ';
            if in_whitespace && !next_whitespace {
                let word = &line[start..idx];
                start = idx;
                in_whitespace = next_whitespace;
                return Some(word);
            }

            in_whitespace = next_whitespace;
        }

        if start < line.len() {
            let word = &line[start..];
            start = line.len();
            return Some(word);
        }

        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_find_words {
        ($ascii_name:ident,
         $([ $line:expr, $ascii_words:expr ]),+) => {
            #[test]
            fn $ascii_name() {
                $(
                    let expected_words: Vec<&str> = $ascii_words.to_vec();
                    let actual_words = find_words_ascii_space($line)
                        .collect::<Vec<_>>();
                    assert_eq!(actual_words, expected_words, "Line: {:?}", $line);
                )+
            }
        };
    }

    test_find_words!(ascii_space_empty, ["", []]);

    test_find_words!(ascii_single_word, ["foo", ["foo"]]);

    test_find_words!(ascii_two_words, ["foo bar", ["foo ", "bar"]]);

    test_find_words!(
        ascii_multiple_words,
        ["foo bar", ["foo ", "bar"]],
        ["x y z", ["x ", "y ", "z"]]
    );

    test_find_words!(ascii_only_whitespace, [" ", [" "]], ["    ", ["    "]]);

    test_find_words!(
        ascii_inter_word_whitespace,
        ["foo   bar", ["foo   ", "bar"]]
    );

    test_find_words!(ascii_trailing_whitespace, ["foo   ", ["foo   "]]);

    test_find_words!(ascii_leading_whitespace, ["   foo", ["   ", "foo"]]);

    test_find_words!(
        ascii_multi_column_char,
        ["\u{1f920}", ["\u{1f920}"]] // cowboy emoji 🤠
    );

    test_find_words!(
        ascii_hyphens,
        ["foo-bar", ["foo-bar"]],
        ["foo- bar", ["foo- ", "bar"]],
        ["foo - bar", ["foo ", "- ", "bar"]],
        ["foo -bar", ["foo ", "-bar"]]
    );

    test_find_words!(ascii_newline, ["foo\nbar", ["foo\nbar"]]);

    test_find_words!(ascii_tab, ["foo\tbar", ["foo\tbar"]]);

    test_find_words!(
        ascii_non_breaking_space,
        ["foo\u{00A0}bar", ["foo\u{00A0}bar"]]
    );
}
