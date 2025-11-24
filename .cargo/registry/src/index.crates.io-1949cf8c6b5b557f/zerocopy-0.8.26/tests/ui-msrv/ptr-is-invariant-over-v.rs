use zerocopy::pointer::{
    invariant::{Aligned, Exclusive, Shared, Valid},
    Ptr,
};

fn _when_exclusive<'big: 'small, 'small>(
    big: Ptr<'small, &'big u32, (Exclusive, Aligned, Valid)>,
    mut _small: Ptr<'small, &'small u32, (Exclusive, Aligned, Valid)>,
) {
    _small = big;
}

fn _when_shared<'big: 'small, 'small>(
    big: Ptr<'small, &'big u32, (Shared, Aligned, Valid)>,
    mut _small: Ptr<'small, &'small u32, (Shared, Aligned, Valid)>,
) {
    _small = big;
}

fn main() {}
