use minimal_lexical::number::Number;

#[test]
fn is_fast_path_test() {
    let mut number = Number {
        exponent: -4,
        mantissa: 12345,
        many_digits: false,
    };
    assert_eq!(number.is_fast_path::<f32>(), true);
    assert_eq!(number.is_fast_path::<f64>(), true);

    number.exponent = -15;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), true);

    number.exponent = -25;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), false);

    number.exponent = 25;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), true);

    number.exponent = 36;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), true);

    number.exponent = 38;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), false);

    number.mantissa = 1 << 25;
    number.exponent = 0;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), true);

    number.mantissa = 1 << 54;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), false);

    number.mantissa = 1 << 52;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), true);

    number.many_digits = true;
    assert_eq!(number.is_fast_path::<f32>(), false);
    assert_eq!(number.is_fast_path::<f64>(), false);
}

#[test]
fn try_fast_path_test() {
    let mut number = Number {
        exponent: -4,
        mantissa: 12345,
        many_digits: false,
    };
    assert_eq!(number.try_fast_path::<f32>(), Some(1.2345));
    assert_eq!(number.try_fast_path::<f64>(), Some(1.2345));

    number.exponent = -10;
    assert_eq!(number.try_fast_path::<f32>(), Some(1.2345e-6));
    assert_eq!(number.try_fast_path::<f64>(), Some(1.2345e-6));

    number.exponent = -20;
    assert_eq!(number.try_fast_path::<f32>(), None);
    assert_eq!(number.try_fast_path::<f64>(), Some(1.2345e-16));

    number.exponent = -25;
    assert_eq!(number.try_fast_path::<f32>(), None);
    assert_eq!(number.try_fast_path::<f64>(), None);

    number.exponent = 12;
    assert_eq!(number.try_fast_path::<f32>(), Some(1.2345e16));
    assert_eq!(number.try_fast_path::<f64>(), Some(1.2345e16));

    number.exponent = 25;
    assert_eq!(number.try_fast_path::<f32>(), None);
    assert_eq!(number.try_fast_path::<f64>(), Some(1.2345e29));

    number.exponent = 32;
    assert_eq!(number.try_fast_path::<f32>(), None);
    assert_eq!(number.try_fast_path::<f64>(), Some(1.2345e36));

    number.exponent = 36;
    assert_eq!(number.try_fast_path::<f32>(), None);
    assert_eq!(number.try_fast_path::<f64>(), None);
}
