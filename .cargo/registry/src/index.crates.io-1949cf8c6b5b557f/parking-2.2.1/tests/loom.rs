#![cfg(loom)]

#[test]
fn smoke() {
    loom::model(|| {
        let (p, u) = parking::pair();

        loom::thread::spawn(move || {
            p.park();
        });

        u.unpark();
    });
}

#[test]
fn yield_then_unpark() {
    loom::model(|| {
        let (p, u) = parking::pair();

        loom::thread::spawn(move || {
            loom::thread::yield_now();
            u.unpark();
        });

        p.park();
    });
}
