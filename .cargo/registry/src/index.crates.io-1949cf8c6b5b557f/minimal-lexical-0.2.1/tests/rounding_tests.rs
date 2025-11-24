use minimal_lexical::extended_float::ExtendedFloat;
use minimal_lexical::rounding;

#[test]
fn round_test() {
    let mut fp = ExtendedFloat {
        mant: 9223372036854776832,
        exp: -10,
    };
    rounding::round::<f64, _>(&mut fp, |f, s| {
        f.mant >>= s;
        f.exp += s;
    });
    assert_eq!(fp.mant, 0);
    assert_eq!(fp.exp, 1);

    let mut fp = ExtendedFloat {
        mant: 9223372036854776832,
        exp: -10,
    };
    rounding::round::<f64, _>(&mut fp, |f, s| {
        f.mant >>= s;
        f.exp += s;
        // Round-up.
        f.mant += 1;
    });
    assert_eq!(fp.mant, 1);
    assert_eq!(fp.exp, 1);

    // Round-down
    let mut fp = ExtendedFloat {
        mant: 9223372036854776832,
        exp: -10,
    };
    rounding::round::<f64, _>(&mut fp, |f, s| {
        rounding::round_nearest_tie_even(f, s, |is_odd, is_halfway, is_above| {
            is_above || (is_odd && is_halfway)
        });
    });
    assert_eq!(fp.mant, 0);
    assert_eq!(fp.exp, 1);

    // Round up
    let mut fp = ExtendedFloat {
        mant: 9223372036854778880,
        exp: -10,
    };
    rounding::round::<f64, _>(&mut fp, |f, s| {
        rounding::round_nearest_tie_even(f, s, |is_odd, is_halfway, is_above| {
            is_above || (is_odd && is_halfway)
        });
    });
    assert_eq!(fp.mant, 2);
    assert_eq!(fp.exp, 1);

    // Round down
    let mut fp = ExtendedFloat {
        mant: 9223372036854778880,
        exp: -10,
    };
    rounding::round::<f64, _>(&mut fp, rounding::round_down);
    assert_eq!(fp.mant, 1);
    assert_eq!(fp.exp, 1);
}
