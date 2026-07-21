/// Environment overrides for Git commands that must only use local repository data.
///
/// An empty `GIT_ALLOW_PROTOCOL` list denies every Git transport, including implicit fetches from
/// promisor remotes. `GIT_NO_LAZY_FETCH` provides defense in depth on Git versions that support it;
/// older versions safely ignore the unknown variable.
pub fn local_only_git_env() -> impl Iterator<Item = (&'static str, &'static str)> {
    [("GIT_ALLOW_PROTOCOL", ""), ("GIT_NO_LAZY_FETCH", "1")].into_iter()
}
