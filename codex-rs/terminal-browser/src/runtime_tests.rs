use pretty_assertions::assert_eq;

use super::BrowserRuntime;
use crate::network::BrowserNetworkPolicy;

#[test]
fn runtime_replaces_the_user_home_and_working_storage() {
    let runtime = BrowserRuntime::create(/*persistent_profile*/ None).expect("create runtime");
    let env = runtime.environment(&BrowserNetworkPolicy::Direct);

    assert_eq!(
        env.get("HOME"),
        Some(&runtime.home.as_path().display().to_string())
    );
    assert_eq!(
        env.get("TMPDIR"),
        Some(&runtime.temporary.as_path().display().to_string())
    );
    assert!(runtime.profile.starts_with(runtime.root.as_path()));
}
