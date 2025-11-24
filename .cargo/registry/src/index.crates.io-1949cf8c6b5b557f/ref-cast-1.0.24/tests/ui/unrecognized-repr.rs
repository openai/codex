use ref_cast::RefCast;

#[derive(RefCast)]
#[repr(packed, C, usize, usize(0), usize = "0")]
struct Test {
    s: String,
}

fn main() {}
