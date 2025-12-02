use codex_execpolicy_legacy::PositiveExampleFailedCheck;
use codex_execpolicy_legacy::get_default_policy;

#[test]
fn verify_everything_in_good_list_is_allowed() {
    let policy = get_default_policy().expect("failed to load default policy");
    let violations = policy.check_each_good_list_individually();
    assert_eq!(Vec::<PositiveExampleFailedCheck>::new(), violations);
}
