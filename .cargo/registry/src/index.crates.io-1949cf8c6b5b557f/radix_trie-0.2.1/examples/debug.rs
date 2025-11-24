extern crate radix_trie;

use radix_trie::Trie;

fn main() {
    let mut trie = Trie::new();
    trie.insert("hello", 19u32);
    trie.insert("hellcat", 35u32);
    trie.insert("not related", 1u32);
    trie.insert("handle nested", 5u32);

    println!("{:#?}", trie);
}
