//! This tests that we compute the same hash as `derive(Hash)`.

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

macro_rules! define {
    ($kw:tt $($rest:tt)*) => {
        #[derive(Derivative)]
        #[derivative(Hash)]
        $kw Ours $($rest)*

        #[derive(Hash)]
        $kw Theirs $($rest)*
    }
}

struct FakeHasher<'a>(&'a mut Vec<u8>);
impl<'a> ::std::hash::Hasher for FakeHasher<'a> {
    fn finish(&self) -> u64 {
        unimplemented!()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.extend(bytes);
    }
}

fn fake_hash<E: ::std::hash::Hash>(e: E) -> Vec<u8> {
    let mut v = Vec::new();
    e.hash(&mut FakeHasher(&mut v));
    v
}

#[test]
fn main() {
    {
        define! {
            struct;
        }

        assert_eq!(fake_hash(Ours), fake_hash(Theirs));
    }

    {
        define! {
            struct {
                foo: u8
            }
        }

        assert_eq!(fake_hash(Ours { foo: 0 }), fake_hash(Theirs { foo: 0 }));
        assert_eq!(fake_hash(Ours { foo: 42 }), fake_hash(Theirs { foo: 42 }));
    }

    {
        define! {
            struct<'a> {
                foo: u8,
                bar: &'a str,
            }
        }

        assert_eq!(fake_hash(Ours { foo: 0, bar: "bar" }), fake_hash(Theirs { foo: 0, bar: "bar" }));
        assert_eq!(fake_hash(Ours { foo: 42, bar: "bar" }), fake_hash(Theirs { foo: 42, bar: "bar" }));
    }

    {
        define! {
            struct<'a> (u8, &'a str);
        }

        assert_eq!(fake_hash(Ours ( 0, "bar" )), fake_hash(Theirs ( 0, "bar" )));
        assert_eq!(fake_hash(Ours ( 42, "bar" )), fake_hash(Theirs ( 42, "bar" )));
    }

    {
        define! {
            enum {
                A, B, C
            }
        }

        assert_eq!(fake_hash(Ours::A), fake_hash(Theirs::A));
        assert_eq!(fake_hash(Ours::B), fake_hash(Theirs::B));
        assert_eq!(fake_hash(Ours::C), fake_hash(Theirs::C));
    }

    {
        define! {
            enum {
                A, B = 42, C
            }
        }

        assert_eq!(fake_hash(Ours::A), fake_hash(Theirs::A));
        assert_eq!(fake_hash(Ours::B), fake_hash(Theirs::B));
        assert_eq!(fake_hash(Ours::C), fake_hash(Theirs::C));
    }

    {
        define! {
            enum {
                A, B = 42, C=1
            }
        }

        assert_eq!(fake_hash(Ours::A), fake_hash(Theirs::A));
        assert_eq!(fake_hash(Ours::B), fake_hash(Theirs::B));
        assert_eq!(fake_hash(Ours::C), fake_hash(Theirs::C));
    }

    {
        #[derive(Derivative)]
        #[derivative(Hash)]
        struct Ours<'a> {
            foo: u8,
            #[derivative(Hash="ignore")]
            bar: &'a str,
            baz: i64,
        }

        #[derive(Hash)]
        struct Theirs {
            foo: u8,
            baz: i64,
        }

        assert_eq!(fake_hash(Ours { foo: 0, bar: "bar", baz: 312 }), fake_hash(Theirs { foo: 0, baz: 312 }));
        assert_eq!(fake_hash(Ours { foo: 42, bar: "bar", baz: 312 }), fake_hash(Theirs { foo: 42, baz: 312 }));
    }
}
