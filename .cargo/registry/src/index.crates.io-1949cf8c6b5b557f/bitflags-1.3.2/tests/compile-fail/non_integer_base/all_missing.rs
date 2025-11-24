use bitflags::bitflags;

struct MyInt(u8);

bitflags! {
    struct Flags128: MyInt {
        const A = MyInt(0b0000_0001);
        const B = MyInt(0b0000_0010);
        const C = MyInt(0b0000_0100);
    }
}

fn main() {}
