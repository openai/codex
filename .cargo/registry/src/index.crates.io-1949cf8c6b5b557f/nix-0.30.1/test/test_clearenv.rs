use std::env;

#[test]
fn clearenv() {
    env::set_var("FOO", "BAR");
    unsafe { nix::env::clearenv() }.unwrap();
    assert_eq!(env::var("FOO").unwrap_err(), env::VarError::NotPresent);
    assert_eq!(env::vars().count(), 0);
}
