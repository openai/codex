use bitflags::bitflags;

bitflags! {
    struct Flags: u32 {
        const A = 0b00000001;
    }
}

impl From<u32> for Flags {
    fn from(v: u32) -> Flags {
        Flags::from_bits_truncate(v)
    }
}

fn main() {

}
