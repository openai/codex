use parking_lot::RwLock;

struct Lock(RwLock<i32>);

#[test]
fn issue_392() {
    let lock = Lock(RwLock::new(0));
    let mut rl = lock.0.upgradable_read();
    rl.with_upgraded(|_| {
        println!("lock upgrade");
    });
    rl.with_upgraded(|_| {
        println!("lock upgrade");
    });
}
