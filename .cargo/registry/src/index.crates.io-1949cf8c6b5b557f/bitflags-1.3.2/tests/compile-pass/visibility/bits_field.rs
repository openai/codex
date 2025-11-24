use bitflags::bitflags;

bitflags! {
    pub struct Flags1: u32 {
        const FLAG_A = 0b00000001;
    }
}

fn main() {
    assert_eq!(0b00000001, Flags1::FLAG_A.bits);
}
