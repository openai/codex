extern crate radix_trie;

use radix_trie::{Trie, TrieCommon};

fn main() {
    let example = "bananaphone".to_string();
    // Our frequency trie will store various strings with the frequency that they are used
    let mut trie: Trie<String, u32> = Trie::new();
    let v: Vec<char> = example.chars().collect();

    // We want the frequencies of all strings from 1 to 3 characters long
    for window_length in 1..4 {
        for a in v.windows(window_length) {
            // Create a new String to hold our key
            let mut s = String::new();
            // Append all chars in a to the String
            s.extend(a);
            // If the value at s exists, add 1. Otherwise, assign 1 as the key's value.
            trie.map_with_default(s, |v| *v += 1, 1);
        }
    }

    // Iterate through all the values in the trie and print them with their frequencies.
    // Iterates and prints in lexicographic order.
    println!("All trie nodes");
    for (k, v) in trie.iter() {
        println!("{}: {}", k, v);
    }

    println!("All children of 'a'");
    for n in trie.subtrie(&"a".to_string()).unwrap().children() {
        println!("{}: {}", n.key().unwrap(), n.value().unwrap());
    }
}
