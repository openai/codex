use crate::runtime::scheduler::inject;

#[test]
fn push_and_pop() {
    const N: usize = 2;

    let (inject, mut synced) = inject::Shared::new();

    for i in 0..N {
        assert_eq!(inject.len(), i);
        let (task, _) = super::unowned(async {});
        unsafe { inject.push(&mut synced, task) };
    }

    for i in 0..N {
        assert_eq!(inject.len(), N - i);
        assert!(unsafe { inject.pop(&mut synced) }.is_some());
    }

    println!("--------------");

    assert!(unsafe { inject.pop(&mut synced) }.is_none());
}

#[test]
fn push_batch_and_pop() {
    let (inject, mut inject_synced) = inject::Shared::new();

    unsafe {
        inject.push_batch(
            &mut inject_synced,
            (0..10).map(|_| super::unowned(async {}).0),
        );

        assert_eq!(5, inject.pop_n(&mut inject_synced, 5).count());
        assert_eq!(5, inject.pop_n(&mut inject_synced, 5).count());
        assert_eq!(0, inject.pop_n(&mut inject_synced, 5).count());
    }
}

#[test]
fn pop_n_drains_on_drop() {
    let (inject, mut inject_synced) = inject::Shared::new();

    unsafe {
        inject.push_batch(
            &mut inject_synced,
            (0..10).map(|_| super::unowned(async {}).0),
        );
        let _ = inject.pop_n(&mut inject_synced, 10);

        assert_eq!(inject.len(), 0);
    }
}
