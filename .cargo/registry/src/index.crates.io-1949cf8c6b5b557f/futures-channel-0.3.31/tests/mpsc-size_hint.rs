use futures::channel::mpsc;
use futures::stream::Stream;

#[test]
fn unbounded_size_hint() {
    let (tx, mut rx) = mpsc::unbounded::<u32>();
    assert_eq!((0, None), rx.size_hint());
    tx.unbounded_send(1).unwrap();
    assert_eq!((1, None), rx.size_hint());
    rx.try_next().unwrap().unwrap();
    assert_eq!((0, None), rx.size_hint());
    tx.unbounded_send(2).unwrap();
    tx.unbounded_send(3).unwrap();
    assert_eq!((2, None), rx.size_hint());
    drop(tx);
    assert_eq!((2, Some(2)), rx.size_hint());
    rx.try_next().unwrap().unwrap();
    assert_eq!((1, Some(1)), rx.size_hint());
    rx.try_next().unwrap().unwrap();
    assert_eq!((0, Some(0)), rx.size_hint());
}

#[test]
fn channel_size_hint() {
    let (mut tx, mut rx) = mpsc::channel::<u32>(10);
    assert_eq!((0, None), rx.size_hint());
    tx.try_send(1).unwrap();
    assert_eq!((1, None), rx.size_hint());
    rx.try_next().unwrap().unwrap();
    assert_eq!((0, None), rx.size_hint());
    tx.try_send(2).unwrap();
    tx.try_send(3).unwrap();
    assert_eq!((2, None), rx.size_hint());
    drop(tx);
    assert_eq!((2, Some(2)), rx.size_hint());
    rx.try_next().unwrap().unwrap();
    assert_eq!((1, Some(1)), rx.size_hint());
    rx.try_next().unwrap().unwrap();
    assert_eq!((0, Some(0)), rx.size_hint());
}
