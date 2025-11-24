#[rustversion::any(
    stable,
    stable(1.34),
    stable(1.34.0),
    beta,
    nightly,
    nightly(2020-02-25),
    since(1.34),
    since(2020-02-25),
    before(1.34),
    before(2020-02-25),
    not(nightly),
    all(stable, beta, nightly),
)]
fn success() {}

#[test]
fn test() {
    success();
}
