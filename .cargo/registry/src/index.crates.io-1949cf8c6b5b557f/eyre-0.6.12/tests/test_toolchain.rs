// These tests check our build script against rustversion.

#[rustversion::attr(not(nightly), ignore)]
#[test]
fn nightlytest() {
    if !cfg!(nightly) {
        panic!("nightly feature isn't set when the toolchain is nightly.");
    }
    if cfg!(any(beta, stable)) {
        panic!("beta, stable, and nightly are mutually exclusive features.")
    }
}

#[rustversion::attr(not(beta), ignore)]
#[test]
fn betatest() {
    if !cfg!(beta) {
        panic!("beta feature is not set when the toolchain is beta.");
    }
    if cfg!(any(nightly, stable)) {
        panic!("beta, stable, and nightly are mutually exclusive features.")
    }
}

#[rustversion::attr(not(stable), ignore)]
#[test]
fn stabletest() {
    if !cfg!(stable) {
        panic!("stable feature is not set when the toolchain is stable.");
    }
    if cfg!(any(nightly, beta)) {
        panic!("beta, stable, and nightly are mutually exclusive features.")
    }
}
