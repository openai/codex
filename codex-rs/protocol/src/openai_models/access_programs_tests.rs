#![cfg(test)]

use super::ModelAccessPrograms;
use super::SavedDaybreakChoice;
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
            (
                programs.daybreak(),
                programs.standard(),
                programs.select_saved(SavedDaybreakChoice::from(Some(true))),
                programs.select_saved(SavedDaybreakChoice::from(Some(false))),
                programs.select_saved(SavedDaybreakChoice::from(/* enabled */ None)),
            ),
            (
                expected_daybreak,
                expected_standard,
                expected_daybreak,
                expected_standard,
                None,
            )
        );
    }
}
