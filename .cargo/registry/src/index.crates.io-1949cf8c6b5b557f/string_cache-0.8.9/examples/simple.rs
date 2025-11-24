

use string_cache::DefaultAtom;

fn main() {
    let mut interned_stuff = Vec::new();
    let text = "here is a sentence of text that will be tokenised and interned and some repeated \
                tokens is of text and";
    for word in text.split_whitespace() {
        let seen_before = interned_stuff
            .iter()
            // We can use impl PartialEq<T> where T is anything string-like to compare to
            // interned strings to either other interned strings, or actual strings  Comparing two
            // interned strings is very fast (normally a single cpu operation).
            .filter(|interned_word| interned_word == &word)
            .count();
        if seen_before > 0 {
            println!(r#"Seen the word "{}" {} times"#, word, seen_before);
        } else {
            println!(r#"Not seen the word "{}" before"#, word);
        }
        // We use the impl From<(Cow<'a, str>, or &'a str, or String) for Atom<Static> to intern a
        // new string
        interned_stuff.push(DefaultAtom::from(word));
    }
}
