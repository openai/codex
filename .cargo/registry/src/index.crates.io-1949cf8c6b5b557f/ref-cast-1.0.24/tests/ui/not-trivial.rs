use ref_cast::RefCast;

#[derive(RefCast)]
#[repr(C)]
struct Test {
    one: String,
    #[trivial]
    two: String,
}

fn main() {}
