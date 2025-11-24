macro_rules! check {
    ($f:tt) => {
        assert_eq!(pretty($f), stringify!($f));
    };
    (-$f:tt) => {
        assert_eq!(pretty(-$f), concat!("-", stringify!($f)));
    };
}
