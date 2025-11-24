extern crate radix_trie;

use radix_trie::*;

fn main() {
    let mut trie = Trie::new();
    let mut key = vec![];
    for i in 0..10_000 {
        key.push(0);
        trie.insert(key.clone(), i);
        key.pop();
        key.push(1);
        trie.insert(key.clone(), i);
    }
    //let res = trie.remove(&blow_stack);
    //println!("{}", res.unwrap());
}
