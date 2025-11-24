extern crate cassowary;

use cassowary::{Variable, Solver, Constraint};
use cassowary::WeightedRelation::*;
use cassowary::strength::*;

mod common;

use common::new_values;

#[test]
fn remove_constraint() {
    let (value_of, update_values) = new_values();

    let mut solver = Solver::new();

    let val = Variable::new();

    let constraint: Constraint = val | EQ(REQUIRED) | 100.0;
    solver.add_constraint(constraint.clone()).unwrap();
    update_values(solver.fetch_changes());

    assert_eq!(value_of(val), 100.0);

    solver.remove_constraint(&constraint).unwrap();
    solver.add_constraint(val | EQ(REQUIRED) | 0.0).unwrap();
    update_values(solver.fetch_changes());

    assert_eq!(value_of(val), 0.0);
}
