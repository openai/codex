use bitflags::bitflags;

// Checks for possible errors caused by overriding names used by `bitflags!` internally.

mod core {}
mod _core {}

bitflags! {
    struct Test: u8 {
        const A = 1;
    }
}

fn main() {}
