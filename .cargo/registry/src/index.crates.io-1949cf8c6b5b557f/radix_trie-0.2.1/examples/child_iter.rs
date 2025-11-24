extern crate radix_trie;

use radix_trie::{Trie, TrieCommon};

fn main() {
    let mut t = Trie::new();
    t.insert("z", 2);
    t.insert("aba", 5);
    t.insert("abb", 6);
    t.insert("abc", 50);

    // This is a bit of a hack that relies on knowing the binary representation of
    // strings... "abd" works, but "abz" doesn't...
    let ab_sum = t.get_raw_ancestor(&"abd").children().fold(0, |acc, c| {
        println!("Iterating over child with value: {:?}", c.value());
        acc + *c.value().unwrap_or(&0)
    });
    println!("{}", ab_sum);
    assert_eq!(ab_sum, 5 + 6 + 50);
}
