struct Thing;

impl Thing {
    fn new() -> Self {
        Thing
    }
}

inventory::collect!(Thing);

inventory::submit!(Thing::new());

fn main() {}
