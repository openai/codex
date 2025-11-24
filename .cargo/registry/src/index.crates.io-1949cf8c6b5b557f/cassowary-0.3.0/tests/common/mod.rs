use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use cassowary::Variable;

#[derive(Clone, Default)]
struct Values(Rc<RefCell<HashMap<Variable, f64>>>);

impl Values {
    fn value_of(&self, var: Variable) -> f64 {
        *self.0.borrow().get(&var).unwrap_or(&0.0)
    }
    fn update_values(&self, changes: &[(Variable, f64)]) {
        for &(ref var, ref value) in changes {
            println!("{:?} changed to {:?}", var, value);
            self.0.borrow_mut().insert(*var, *value);
        }
    }
}

pub fn new_values() -> (Box<Fn(Variable) -> f64>, Box<Fn(&[(Variable, f64)])>) {
    let values = Values(Rc::new(RefCell::new(HashMap::new())));
    let value_of = {
        let values = values.clone();
        move |v| values.value_of(v)
    };
    let update_values = {
        let values = values.clone();
        move |changes: &[_]| {
            values.update_values(changes);
        }
    };
    (Box::new(value_of), Box::new(update_values))
}