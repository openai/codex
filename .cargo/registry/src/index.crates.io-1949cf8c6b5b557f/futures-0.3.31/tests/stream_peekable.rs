use futures::executor::block_on;
use futures::pin_mut;
use futures::stream::{self, Peekable, StreamExt};

#[test]
fn peekable() {
    block_on(async {
        let peekable: Peekable<_> = stream::iter(vec![1u8, 2, 3]).peekable();
        pin_mut!(peekable);
        assert_eq!(peekable.as_mut().peek().await, Some(&1u8));
        assert_eq!(peekable.collect::<Vec<u8>>().await, vec![1, 2, 3]);

        let s = stream::once(async { 1 }).peekable();
        pin_mut!(s);
        assert_eq!(s.as_mut().peek().await, Some(&1u8));
        assert_eq!(s.collect::<Vec<u8>>().await, vec![1]);
    });
}

#[test]
fn peekable_mut() {
    block_on(async {
        let s = stream::iter(vec![1u8, 2, 3]).peekable();
        pin_mut!(s);
        if let Some(p) = s.as_mut().peek_mut().await {
            if *p == 1 {
                *p = 5;
            }
        }
        assert_eq!(s.collect::<Vec<_>>().await, vec![5, 2, 3]);
    });
}

#[test]
fn peekable_next_if_eq() {
    block_on(async {
        // first, try on references
        let s = stream::iter(vec!["Heart", "of", "Gold"]).peekable();
        pin_mut!(s);
        // try before `peek()`
        assert_eq!(s.as_mut().next_if_eq(&"trillian").await, None);
        assert_eq!(s.as_mut().next_if_eq(&"Heart").await, Some("Heart"));
        // try after peek()
        assert_eq!(s.as_mut().peek().await, Some(&"of"));
        assert_eq!(s.as_mut().next_if_eq(&"of").await, Some("of"));
        assert_eq!(s.as_mut().next_if_eq(&"zaphod").await, None);
        // make sure `next()` still behaves
        assert_eq!(s.next().await, Some("Gold"));

        // make sure comparison works for owned values
        let s = stream::iter(vec![String::from("Ludicrous"), "speed".into()]).peekable();
        pin_mut!(s);
        // make sure basic functionality works
        assert_eq!(s.as_mut().next_if_eq("Ludicrous").await, Some("Ludicrous".into()));
        assert_eq!(s.as_mut().next_if_eq("speed").await, Some("speed".into()));
        assert_eq!(s.as_mut().next_if_eq("").await, None);
    });
}
