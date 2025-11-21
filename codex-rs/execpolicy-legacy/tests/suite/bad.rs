use codex_execpolicy_legacy::NegativeExamplePassedCheck;
use codex_execpolicy_legacy::get_default_policy;

#[test]
fn verify_everything_in_bad_list_is_rejected() {
    let policy = get_default_policy().expect("failed to load default policy");
    let violations = policy.check_each_bad_list_individually();
    assert_eq!(Vec::<NegativeExamplePassedCheck>::new(), violations);
}
