use futures::executor::block_on;
use futures::future::{self, FutureExt};

#[test]
fn smoke() {
    let mut counter = 0;

    {
        let work = future::ready::<i32>(40).inspect(|val| {
            counter += *val;
        });
        assert_eq!(block_on(work), 40);
    }

    assert_eq!(counter, 40);
}
