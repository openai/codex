use bitflags::bitflags;

bitflags! {
    pub struct Flags1: u32 {
        pub const FLAG_A = 0b00000001;
    }
}

fn main() {}
