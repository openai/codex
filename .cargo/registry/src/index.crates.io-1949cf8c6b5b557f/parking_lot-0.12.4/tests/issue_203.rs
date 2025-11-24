use parking_lot::RwLock;
use std::thread;

struct Bar(RwLock<()>);

impl Drop for Bar {
    fn drop(&mut self) {
        let _n = self.0.write();
    }
}

thread_local! {
    static B: Bar = Bar(RwLock::new(()));
}

#[test]
fn main() {
    thread::spawn(|| {
        B.with(|_| ());

        let a = RwLock::new(());
        let _a = a.read();
    })
    .join()
    .unwrap();
}
