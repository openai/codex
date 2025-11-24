use bitflags::bitflags;

// Checks for possible errors caused by overriding names used by `bitflags!` internally.

#[allow(unused_macros)]
macro_rules! stringify {
    ($($t:tt)*) => { "..." };
}

bitflags! {
    struct Test: u8 {
        const A = 1;
    }
}

fn main() {
    // Just make sure we don't call the redefined `stringify` macro
    assert_eq!(format!("{:?}", Test::A), "A");
}
