//! Tests for iteration over receivers.

use crossbeam_channel::unbounded;
use crossbeam_utils::thread::scope;

#[test]
fn nested_recv_iter() {
    let (s, r) = unbounded::<i32>();
    let (total_s, total_r) = unbounded::<i32>();

    scope(|scope| {
        scope.spawn(move |_| {
            let mut acc = 0;
            for x in r.iter() {
                acc += x;
            }
            total_s.send(acc).unwrap();
        });

        s.send(3).unwrap();
        s.send(1).unwrap();
        s.send(2).unwrap();
        drop(s);
        assert_eq!(total_r.recv().unwrap(), 6);
    })
    .unwrap();
}

#[test]
fn recv_iter_break() {
    let (s, r) = unbounded::<i32>();
    let (count_s, count_r) = unbounded();

    scope(|scope| {
        scope.spawn(move |_| {
            let mut count = 0;
            for x in r.iter() {
                if count >= 3 {
                    break;
                } else {
                    count += x;
                }
            }
            count_s.send(count).unwrap();
        });

        s.send(2).unwrap();
        s.send(2).unwrap();
        s.send(2).unwrap();
        let _ = s.send(2);
        drop(s);
        assert_eq!(count_r.recv().unwrap(), 4);
    })
    .unwrap();
}

#[test]
fn recv_try_iter() {
    let (request_s, request_r) = unbounded();
    let (response_s, response_r) = unbounded();

    scope(|scope| {
        scope.spawn(move |_| {
            let mut count = 0;
            loop {
                for x in response_r.try_iter() {
                    count += x;
                    if count == 6 {
                        return;
                    }
                }
                request_s.send(()).unwrap();
            }
        });

        for _ in request_r.iter() {
            if response_s.send(2).is_err() {
                break;
            }
        }
    })
    .unwrap();
}

#[test]
fn recv_into_iter_owned() {
    let mut iter = {
        let (s, r) = unbounded::<i32>();
        s.send(1).unwrap();
        s.send(2).unwrap();
        r.into_iter()
    };

    assert_eq!(iter.next().unwrap(), 1);
    assert_eq!(iter.next().unwrap(), 2);
    assert!(iter.next().is_none());
}

#[test]
fn recv_into_iter_borrowed() {
    let (s, r) = unbounded::<i32>();
    s.send(1).unwrap();
    s.send(2).unwrap();
    drop(s);

    let mut iter = (&r).into_iter();
    assert_eq!(iter.next().unwrap(), 1);
    assert_eq!(iter.next().unwrap(), 2);
    assert!(iter.next().is_none());
}
