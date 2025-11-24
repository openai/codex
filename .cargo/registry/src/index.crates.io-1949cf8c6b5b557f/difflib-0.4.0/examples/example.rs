extern crate difflib;

use difflib::differ::Differ;
use difflib::sequencematcher::SequenceMatcher;

fn main() {
    // unified_diff
    let first_text = "one two three four".split(" ").collect::<Vec<&str>>();
    let second_text = "zero one tree four".split(" ").collect::<Vec<&str>>();
    let diff = difflib::unified_diff(
        &first_text,
        &second_text,
        "Original",
        "Current",
        "2005-01-26 23:30:50",
        "2010-04-02 10:20:52",
        3,
    );
    for line in &diff {
        println!("{:?}", line);
    }

    //context_diff
    let diff = difflib::context_diff(
        &first_text,
        &second_text,
        "Original",
        "Current",
        "2005-01-26 23:30:50",
        "2010-04-02 10:20:52",
        3,
    );
    for line in &diff {
        println!("{:?}", line);
    }

    //get_close_matches
    let words = vec!["ape", "apple", "peach", "puppy"];
    let result = difflib::get_close_matches("appel", words, 3, 0.6);
    println!("{:?}", result);

    //Differ examples
    let differ = Differ::new();
    let diff = differ.compare(&first_text, &second_text);
    for line in &diff {
        println!("{:?}", line);
    }

    //SequenceMatcher examples
    let mut matcher = SequenceMatcher::new("one two three four", "zero one tree four");
    let m = matcher.find_longest_match(0, 18, 0, 18);
    println!("{:?}", m);
    let all_matches = matcher.get_matching_blocks();
    println!("{:?}", all_matches);
    let opcode = matcher.get_opcodes();
    println!("{:?}", opcode);
    let grouped_opcodes = matcher.get_grouped_opcodes(2);
    println!("{:?}", grouped_opcodes);
    let ratio = matcher.ratio();
    println!("{:?}", ratio);
    matcher.set_seqs("aaaaa", "aaaab");
    println!("{:?}", matcher.ratio());
}
