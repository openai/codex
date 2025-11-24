use bitflags::bitflags;

bitflags! {
    #[repr(C)]
    struct Flags: u32 {
        const A = 0b00000001;
    }
}

fn main() {}
