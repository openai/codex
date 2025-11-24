use ref_cast::ref_cast_custom;
use ref_cast_test_suite::Struct;

#[ref_cast_custom]
fn ref_cast(s: &str) -> &Struct;

#[ref_cast_custom]
fn ref_cast_mut(s: &mut str) -> &mut Struct;

fn main() {}
