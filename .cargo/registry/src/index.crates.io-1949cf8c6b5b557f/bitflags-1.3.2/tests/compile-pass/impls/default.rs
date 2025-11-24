use bitflags::bitflags;

bitflags! {
    #[derive(Default)]
    struct Flags: u32 {
        const A = 0b00000001;
    }
}

fn main() {}
