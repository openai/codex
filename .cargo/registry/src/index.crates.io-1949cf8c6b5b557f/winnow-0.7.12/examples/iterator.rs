use std::collections::HashMap;
use std::iter::Iterator;

use winnow::ascii::alphanumeric1;
use winnow::combinator::iterator;
use winnow::combinator::{separated_pair, terminated};
use winnow::prelude::*;

fn main() {
    let mut data = "abcabcabcabc";

    fn parser<'s>(i: &mut &'s str) -> ModalResult<&'s str> {
        "abc".parse_next(i)
    }

    // `from_fn` (available from Rust 1.34) can create an iterator
    // from a closure
    let it = std::iter::from_fn(move || parser.parse_next(&mut data).ok());

    for value in it {
        println!("parser returned: {value}");
    }

    println!("\n********************\n");

    let mut data = "abcabcabcabc";

    // if `from_fn` is not available, it is possible to fold
    // over an iterator of functions
    let res = std::iter::repeat(parser)
        .take(3)
        .try_fold(Vec::new(), |mut acc, mut parser| {
            parser.parse_next(&mut data).map(|o| {
                acc.push(o);
                acc
            })
        });

    // will print "parser iterator returned: Ok(("abc", ["abc", "abc", "abc"]))"
    println!("\nparser iterator returned: {res:?}");

    println!("\n********************\n");

    let data = "key1:value1,key2:value2,key3:value3,;";

    // `winnow::combinator::iterator` will return an iterator
    // producing the parsed values. Compared to the previous
    // solutions:
    // - we can work with a normal iterator like `from_fn`
    // - we can get the remaining input afterwards, like with the `try_fold` trick
    let mut winnow_it = iterator(
        data,
        terminated(separated_pair(alphanumeric1, ":", alphanumeric1), ","),
    );

    let res = winnow_it
        .map(|(k, v)| (k.to_uppercase(), v))
        .collect::<HashMap<_, _>>();

    let parser_result: ModalResult<(_, _), ()> = winnow_it.finish();
    let (remaining_input, ()) = parser_result.unwrap();

    // will print "iterator returned {"key1": "value1", "key3": "value3", "key2": "value2"}, remaining input is ';'"
    println!("iterator returned {res:?}, remaining input is '{remaining_input}'");
}
