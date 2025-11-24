use std::ops::RangeInclusive;

struct SegmentAllowedValues<'a> {
    /// Range of permitted numeric values
    numeric_range: RangeInclusive<u64>,

    /// Allowed alphabetic single values
    single_values: &'a [&'a str],
}

const MONTHS: &[&str] = &[
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

const DAYS: &[&str] = &["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

const ALLOWED_VALUES: &[&SegmentAllowedValues] = &[
    &SegmentAllowedValues {
        numeric_range: 0..=59,
        single_values: &[],
    },
    &SegmentAllowedValues {
        numeric_range: 0..=23,
        single_values: &[],
    },
    &SegmentAllowedValues {
        numeric_range: 1..=31,
        single_values: &[],
    },
    &SegmentAllowedValues {
        numeric_range: 1..=12,
        single_values: MONTHS,
    },
    &SegmentAllowedValues {
        numeric_range: 0..=6,
        single_values: DAYS,
    },
];

fn validate_range(range: &str, allowed_values: &SegmentAllowedValues) -> bool {
    if range == "*" {
        return true;
    }

    let range_limits: Vec<_> = range.split('-').map(str::parse::<u64>).collect();

    range_limits.len() == 2
        && range_limits.iter().all(|limit| match limit {
            Ok(limit) => allowed_values.numeric_range.contains(limit),
            Err(_) => false,
        })
        && range_limits[0].as_ref().unwrap() <= range_limits[1].as_ref().unwrap()
}

fn validate_step(step: &str) -> bool {
    match step.parse::<u64>() {
        Ok(value) => value > 0,
        Err(_) => false,
    }
}

fn validate_steprange(steprange: &str, allowed_values: &SegmentAllowedValues) -> bool {
    let mut steprange_split = steprange.splitn(2, '/');
    let range_is_valid = match steprange_split.next() {
        Some(range) => validate_range(range, allowed_values),
        None => false,
    };

    range_is_valid
        && match steprange_split.next() {
            Some(step) => validate_step(step),
            None => true,
        }
}

fn validate_listitem(listitem: &str, allowed_values: &SegmentAllowedValues) -> bool {
    match listitem.parse::<u64>() {
        Ok(value) => allowed_values.numeric_range.contains(&value),
        Err(_) => validate_steprange(listitem, allowed_values),
    }
}

fn validate_list(list: &str, allowed_values: &SegmentAllowedValues) -> bool {
    list.split(',')
        .all(|listitem| validate_listitem(listitem, allowed_values))
}

fn validate_segment(segment: &str, allowed_values: &SegmentAllowedValues) -> bool {
    allowed_values
        .single_values
        .contains(&segment.to_lowercase().as_ref())
        || validate_list(segment, allowed_values)
}

pub fn validate(crontab: &str) -> bool {
    let lists: Vec<_> = crontab.split_whitespace().collect();
    if lists.len() != 5 {
        return false;
    }

    lists
        .iter()
        .zip(ALLOWED_VALUES)
        .all(|(segment, allowed_values)| validate_segment(segment, allowed_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("* * * * *", true)]
    #[case(" *  *  *      *    * ", true)]
    #[case("invalid", false)]
    #[case("", false)]
    #[case("* * * *", false)]
    #[case("* * * * * *", false)]
    #[case("0 0 1 1 0", true)]
    #[case("0 0 0 1 0", false)]
    #[case("0 0 1 0 0", false)]
    #[case("59 23 31 12 6", true)]
    #[case("0 0 1 may sun", true)]
    #[case("0 0 1 may sat,sun", false)]
    #[case("0 0 1 may,jun sat", false)]
    #[case("0 0 1 fri sun", false)]
    #[case("0 0 1 JAN WED", true)]
    #[case("0,24 5,23,6 1,2,3,31 1,2 5,6", true)]
    #[case("0-20 * * * *", true)]
    #[case("20-0 * * * *", false)]
    #[case("0-20/3 * * * *", true)]
    #[case("20/3 * * * *", false)]
    #[case("*/3 * * * *", true)]
    #[case("*/3,2 * * * *", true)]
    #[case("*/foo * * * *", false)]
    #[case("1-foo * * * *", false)]
    #[case("foo-34 * * * *", false)]
    fn test_parse(#[case] crontab: &str, #[case] expected: bool) {
        assert_eq!(
            validate(crontab),
            expected,
            "\"{crontab}\" is {}a valid crontab",
            match expected {
                true => "",
                false => "not ",
            },
        );
    }
}
