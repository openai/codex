use ref_cast::RefCast;

#[derive(RefCast)]
#[repr(align(2), C, align = "2")]
struct Test {
    s: String,
}

fn main() {}
