use predicates::prelude::*;
use predicates_tree::CaseTreeExt;

fn main() {
    let pred = predicate::ne(5).not().and(predicate::ge(5));

    let var = 5;
    let case = pred.find_case(true, &var);
    if let Some(case) = case {
        println!("var is {var}");
        println!("{}", case.tree());
    }
}
