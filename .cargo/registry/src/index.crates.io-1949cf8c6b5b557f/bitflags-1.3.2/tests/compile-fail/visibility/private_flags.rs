mod example {
    use bitflags::bitflags;

    bitflags! {
        pub struct Flags1: u32 {
            const FLAG_A = 0b00000001;
        }

        struct Flags2: u32 {
            const FLAG_B = 0b00000010;
        }
    }
}

fn main() {
    let flag1 = example::Flags1::FLAG_A;
    let flag2 = example::Flags2::FLAG_B;
}
