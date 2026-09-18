//! Choose tests by the contract that changes: the owning event suite covers fact-to-JSON
//! behavior, privacy, and prerequisites; client protocol covers admission; client delivery covers
//! authentication, batching, and destinations; reducer ordering covers relative arrivals across
//! owners. Use owner-local tests for private invariants when needed, and Core or app-server
//! integration tests when changing the actual producer or emission. Client tests live in this
//! directory but are mounted by `client.rs` for private access without widening internals.

#[path = "approvals_tests.rs"]
mod approvals;
#[path = "apps_tests.rs"]
mod apps;
#[path = "hooks_tests.rs"]
mod hooks;
#[path = "onboarding_tests.rs"]
mod onboarding;
#[path = "plugins_tests.rs"]
mod plugins;
#[path = "reducer_ordering_tests.rs"]
mod reducer_ordering;
#[path = "skills_tests.rs"]
mod skills;
#[path = "threads_tests.rs"]
mod threads;
#[path = "tools_tests.rs"]
mod tools;
#[path = "turns_tests.rs"]
mod turns;
