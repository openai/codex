use super::ModelAccessPrograms;
use crate::turn_input::CyberAccessProgram;
use pretty_assertions::assert_eq;

#[test]
fn selects_only_advertised_programs_with_blue_preferred() {
    use CyberAccessProgram::*;
    for (cyber, expected_daybreak, expected_standard) in [
        (
            vec![Standard, DaybreakRed, DaybreakBlue],
            Some(DaybreakBlue),
            Some(Standard),
        ),
        (vec![DaybreakRed], Some(DaybreakRed), None),
        (vec![Standard], None, Some(Standard)),
        (vec![], None, None),
    ] {
        let programs = ModelAccessPrograms { cyber };
        assert_eq!(
            (programs.daybreak(), programs.standard()),
            (expected_daybreak, expected_standard)
        );
    }
}
