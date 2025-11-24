use itertools::Itertools;

struct PanickingCounter {
    curr: usize,
    max: usize,
}

impl Iterator for PanickingCounter {
    type Item = ();

    fn next(&mut self) -> Option<Self::Item> {
        self.curr += 1;

        assert_ne!(
            self.curr, self.max,
            "Input iterator reached maximum of {} suggesting collection by adaptor",
            self.max
        );

        Some(())
    }
}

fn no_collect_test<A, T>(to_adaptor: T)
    where A: Iterator, T: Fn(PanickingCounter) -> A
{
    let counter = PanickingCounter { curr: 0, max: 10_000 };
    let adaptor = to_adaptor(counter);

    for _ in adaptor.take(5) {}
}

#[test]
fn permutations_no_collect() {
    no_collect_test(|iter| iter.permutations(5))
}

#[test]
fn combinations_no_collect() {
    no_collect_test(|iter| iter.combinations(5))
}

#[test]
fn combinations_with_replacement_no_collect() {
    no_collect_test(|iter| iter.combinations_with_replacement(5))
}