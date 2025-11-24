use predicates::prelude::*;
use predicates_tree::CaseTreeExt;

fn main() {
    let expected = 10;
    let actual = 15;
    let pred = predicates::ord::eq(expected);
    if let Some(case) = pred.find_case(false, &actual) {
        let tree = case.tree();
        println!("{}", tree);
    }

    let expected = [1, 2, 3];
    let actual = 15;
    let pred = predicates::iter::in_iter(IntoIterator::into_iter(expected));
    if let Some(case) = pred.find_case(false, &actual) {
        let tree = case.tree();
        println!("{}", tree);
    }

    let expected = "Hello
World!

Goodbye!";
    let actual = "Hello
Moon!

Goodbye!";
    let pred = predicates::ord::eq(expected);
    if let Some(case) = pred.find_case(false, &actual) {
        let tree = case.tree();
        println!("{}", tree);
    }

    let expected = "Hello
World!

Goodbye!";
    let actual = "Hello
Moon!

Goodbye!";
    let pred = predicates::str::diff(expected);
    if let Some(case) = pred.find_case(false, actual) {
        let tree = case.tree();
        println!("{}", tree);
    }
}
