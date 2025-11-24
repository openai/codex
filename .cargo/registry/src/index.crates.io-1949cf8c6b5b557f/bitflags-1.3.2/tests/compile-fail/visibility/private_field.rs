mod example {
    use bitflags::bitflags;

    bitflags! {
        pub struct Flags1: u32 {
            const FLAG_A = 0b00000001;
        }
    }
}

fn main() {
    let flag1 = example::Flags1::FLAG_A.bits;
}
