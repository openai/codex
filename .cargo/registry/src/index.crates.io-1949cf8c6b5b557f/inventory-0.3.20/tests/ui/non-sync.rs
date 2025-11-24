use std::rc::Rc;
use std::thread;

struct Thing(Rc<i32>);

inventory::collect!(Thing);

fn clone_all() {
    for thing in inventory::iter::<Thing> {
        let _ = Rc::clone(&thing.0);
    }
}

fn main() {
    // It would be bad if this were allowed. These threads would race on the
    // nonatomic reference counts.
    let thread1 = thread::spawn(clone_all);
    let thread2 = thread::spawn(clone_all);
    thread1.join().unwrap();
    thread2.join().unwrap();
}
