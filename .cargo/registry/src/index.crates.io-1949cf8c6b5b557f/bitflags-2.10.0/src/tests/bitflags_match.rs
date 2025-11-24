bitflags! {
    #[derive(PartialEq)]
    struct Flags: u8 {
        const A = 1 << 0;
        const B = 1 << 1;
        const C = 1 << 2;
        const D = 1 << 3;
    }
}

fn flag_to_string(flag: Flags) -> String {
    bitflags_match!(flag, {
        Flags::A => "A".to_string(),
        Flags::B => { "B".to_string() }
        Flags::C => "C".to_string(),
        Flags::D => "D".to_string(),
        Flags::A | Flags::B => "A or B".to_string(),
        Flags::A & Flags::B => { "A and B | empty".to_string() },
        Flags::A ^ Flags::B => "A xor B".to_string(),
        Flags::A | Flags::B | Flags::C => "A or B or C".to_string(),
        Flags::A & Flags::B & Flags::C => "A and B and C".to_string(),
        Flags::A ^ Flags::B ^ Flags::C => "A xor B xor C".to_string(),
        Flags::A | Flags::B | Flags::C | Flags::D => "All flags".to_string(),
        _ => "Unknown combination".to_string()
    })
}

#[test]
fn test_single_flags() {
    assert_eq!(flag_to_string(Flags::A), "A");
    assert_eq!(flag_to_string(Flags::B), "B");
    assert_eq!(flag_to_string(Flags::C), "C");
    assert_eq!(flag_to_string(Flags::D), "D");
}

#[test]
fn test_or_operations() {
    assert_eq!(flag_to_string(Flags::A | Flags::B), "A or B");
    assert_eq!(
        flag_to_string(Flags::A | Flags::B | Flags::C),
        "A or B or C"
    );
    assert_eq!(
        flag_to_string(Flags::A | Flags::B | Flags::C | Flags::D),
        "All flags"
    );
}

#[test]
fn test_and_operations() {
    assert_eq!(flag_to_string(Flags::A & Flags::A), "A");
    assert_eq!(flag_to_string(Flags::A & Flags::B), "A and B | empty");
    assert_eq!(
        flag_to_string(Flags::A & Flags::B & Flags::C),
        "A and B | empty"
    ); // Since A, B, and C are mutually exclusive, the result of A & B & C is 0 ==> A & B & C = 0000 (i.e., empty).
       // However, in the bitflags_match! statement (actually is if {..} else if {..} .. else {..}),
       // the "A & B = 0000" condition is listed first, so 0000 will match "A & B" first,
       // resulting in the output of the "A and B | empty" branch.
    assert_eq!(
        flag_to_string(Flags::A & Flags::B & Flags::C & Flags::D),
        "A and B | empty"
    );
}

#[test]
fn test_xor_operations() {
    assert_eq!(flag_to_string(Flags::A ^ Flags::B), "A or B"); // A | B = A ^ B == 0011
    assert_eq!(flag_to_string(Flags::A ^ Flags::A), "A and B | empty");
    assert_eq!(
        flag_to_string(Flags::A ^ Flags::B ^ Flags::C),
        "A or B or C"
    );
}

#[test]
fn test_complex_operations() {
    assert_eq!(flag_to_string(Flags::A | (Flags::B & Flags::C)), "A");
    assert_eq!(
        flag_to_string((Flags::A | Flags::B) & (Flags::B | Flags::C)),
        "B"
    );
    assert_eq!(
        flag_to_string(Flags::A ^ (Flags::B | Flags::C)),
        "A or B or C"
    );
}

#[test]
fn test_empty_and_full_flags() {
    assert_eq!(flag_to_string(Flags::empty()), "A and B | empty");
    assert_eq!(flag_to_string(Flags::all()), "All flags");
}
