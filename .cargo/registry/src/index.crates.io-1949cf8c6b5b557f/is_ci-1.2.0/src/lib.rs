use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

static INIT: Once = Once::new();
static IS_CI: AtomicBool = AtomicBool::new(false);

/// Returns true if the current environment is found to probably be a CI
/// environment or service. That's it, that's all it does.
#[deprecated(since = "1.1.0", note = "Use `cached` or `uncached` instead")]
pub fn is_ci() -> bool {
    uncached()
}

/// Returns true if the current environment is found to probably be a CI
/// environment or service, and caches the result for future calls. If you
/// expect the environment to change, use [uncached].
pub fn cached() -> bool {
    INIT.call_once(|| IS_CI.store(uncached(), Ordering::Relaxed));
    IS_CI.load(Ordering::Relaxed)
}

/// Returns true if the current environment is found to probably be a CI
/// environment or service. If you expect to call this multiple times without
/// the environment changing, use [cached].
pub fn uncached() -> bool {
    let ci_var = std::env::var("CI");
    ci_var == Ok("true".into())
        || ci_var == Ok("1".into())
        || ci_var == Ok("woodpecker".into())
        || check("CI_NAME")
        || check("GITHUB_ACTION")
        || check("GITLAB_CI")
        || check("NETLIFY")
        || check("TRAVIS")
        || matches!(std::env::var("NODE"), Ok(node) if node.ends_with("//heroku/node/bin/node"))
        || check("CODEBUILD_SRC_DIR")
        || check("BUILDER_OUTPUT")
        || check("GITLAB_DEPLOYMENT")
        || check("NOW_GITHUB_DEPLOYMENT")
        || check("NOW_BUILDER")
        || check("BITBUCKET_DEPLOYMENT")
        || check("GERRIT_PROJECT")
        || check("SYSTEM_TEAMFOUNDATIONCOLLECTIONURI")
        || check("BITRISE_IO")
        || check("BUDDY_WORKSPACE_ID")
        || check("BUILDKITE")
        || check("CIRRUS_CI")
        || check("APPVEYOR")
        || check("CIRCLECI")
        || check("SEMAPHORE")
        || check("DRONE")
        || check("DSARI")
        || check("TDDIUM")
        || check("STRIDER")
        || check("TASKCLUSTER_ROOT_URL")
        || check("JENKINS_URL")
        || check("bamboo.buildKey")
        || check("GO_PIPELINE_NAME")
        || check("HUDSON_URL")
        || check("WERCKER")
        || check("MAGNUM")
        || check("NEVERCODE")
        || check("RENDER")
        || check("SAIL_CI")
        || check("SHIPPABLE")
}

fn check(name: &str) -> bool {
    std::env::var(name).is_ok()
}
