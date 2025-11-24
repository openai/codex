use bitflags::bitflags;

bitflags! {
    struct Flags: u32 {
        const A = 0b00000001;
    }
}

impl Flags {
    pub fn new() -> Flags {
        Flags::A
    }
}

fn main() {}
