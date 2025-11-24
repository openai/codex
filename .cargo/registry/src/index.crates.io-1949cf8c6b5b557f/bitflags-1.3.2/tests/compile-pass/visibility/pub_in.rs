mod a {
    mod b {
        use bitflags::bitflags;

        bitflags! {
            pub(in crate::a) struct Flags: u32 {
                const FLAG_A = 0b00000001;
            }
        }
    }

    pub fn flags() -> u32 {
        b::Flags::FLAG_A.bits()
    }
}

fn main() {
    assert_eq!(0b00000001, a::flags());
}
