use crate::config::NetworkProxyConfig;
use crate::policy::normalize_host;
use anyhow::Context as _;
use anyhow::Result;
use anyhow::anyhow;
use codex_utils_absolute_path::AbsolutePathBuf;
use globset::GlobBuilder;
use globset::GlobMatcher;
use rama_http::HeaderValue;
use rama_http::Request;
use rama_http::header::HeaderName;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use url::form_urlencoded;

const PATTERN_PREFIX: &str = "pattern:";
const LITERAL_PREFIX: &str = "literal:";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct MitmHookConfig {
    pub host: String,
    #[serde(rename = "match", default)]
    pub matcher: MitmHookMatchConfig,
    #[serde(default)]
    pub actions: MitmHookActionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct MitmHookMatchConfig {
    pub methods: Vec<String>,
    pub path_prefixes: Vec<String>,
    pub query: BTreeMap<String, Vec<String>>,
    pub headers: BTreeMap<String, Vec<String>>,
    pub body: Option<MitmHookBodyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct MitmHookActionsConfig {
    pub strip_request_headers: Vec<String>,
    pub inject_request_headers: Vec<InjectedHeaderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct InjectedHeaderConfig {
    pub name: String,
    pub secret_env_var: Option<String>,
    pub secret_file: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct MitmHookBodyConfig(pub serde_json::Value);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MitmHook {
    pub host: String,
    pub matcher: MitmHookMatcher,
    pub actions: MitmHookActions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MitmHookMatcher {
    pub methods: Vec<String>,
    pub path_prefixes: Vec<PathMatcher>,
    pub query: Vec<QueryConstraint>,
    pub headers: Vec<HeaderConstraint>,
    pub body: Option<MitmHookBodyMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryConstraint {
    pub name: String,
    pub allowed_values: Vec<ValueMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderConstraint {
    pub name: HeaderName,
    pub allowed_values: Vec<ValueMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MitmHookActions {
    pub strip_request_headers: Vec<HeaderName>,
    pub inject_request_headers: Vec<ResolvedInjectedHeader>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedInjectedHeader {
    pub name: HeaderName,
    pub value: HeaderValue,
    pub source: SecretSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretSource {
    EnvVar(String),
    File(AbsolutePathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MitmHookBodyMatcher {
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathMatcher {
    Prefix(String),
    Glob(CompiledGlobMatcher),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueMatcher {
    Exact(String),
    Glob(CompiledGlobMatcher),
}

enum MatcherPattern<'a> {
    Literal(&'a str),
    Glob(&'a str),
}

#[derive(Clone)]
pub struct CompiledGlobMatcher {
    pattern: String,
    matcher: GlobMatcher,
}

impl std::fmt::Debug for CompiledGlobMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledGlobMatcher")
            .field("pattern", &self.pattern)
            .finish()
    }
}

impl PartialEq for CompiledGlobMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for CompiledGlobMatcher {}

impl CompiledGlobMatcher {
    fn is_match(&self, candidate: &str) -> bool {
        self.matcher.is_match(candidate)
    }
}

pub type MitmHooksByHost = BTreeMap<String, Vec<MitmHook>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookEvaluation {
    NoHooksForHost,
    Matched { actions: MitmHookActions },
    HookedHostNoMatch,
}

pub(crate) fn validate_mitm_hook_config(config: &NetworkProxyConfig) -> Result<()> {
    let hooks = &config.network.mitm_hooks;
    if hooks.is_empty() {
        return Ok(());
    }

    if !config.network.mitm {
        return Err(anyhow!("network.mitm_hooks requires network.mitm = true"));
    }

    for (hook_index, hook) in hooks.iter().enumerate() {
        let host = normalize_hook_host(&hook.host)
            .with_context(|| format!("invalid network.mitm_hooks[{hook_index}].host"))?;

        let methods = normalize_methods(&hook.matcher.methods)
            .with_context(|| format!("invalid network.mitm_hooks[{hook_index}].match.methods"))?;
        if methods.is_empty() {
            return Err(anyhow!(
                "network.mitm_hooks[{hook_index}].match.methods must not be empty"
            ));
        }

        let path_prefixes =
            compile_path_matchers(&hook.matcher.path_prefixes).with_context(|| {
                format!("invalid network.mitm_hooks[{hook_index}].match.path_prefixes")
            })?;
        if path_prefixes.is_empty() {
            return Err(anyhow!(
                "network.mitm_hooks[{hook_index}].match.path_prefixes must not be empty"
            ));
        }

        if let Some(body) = hook.matcher.body.as_ref() {
            let _ = body;
            return Err(anyhow!(
                "network.mitm_hooks[{hook_index}].match.body is reserved for a future release and is not yet supported"
            ));
        }

        validate_query_constraints(&hook.matcher.query)
            .with_context(|| format!("invalid network.mitm_hooks[{hook_index}].match.query"))?;
        validate_header_constraints(&hook.matcher.headers)
            .with_context(|| format!("invalid network.mitm_hooks[{hook_index}].match.headers"))?;
        validate_strip_request_headers(&hook.actions.strip_request_headers).with_context(|| {
            format!("invalid network.mitm_hooks[{hook_index}].actions.strip_request_headers")
        })?;
        validate_injected_headers(&hook.actions.inject_request_headers).with_context(|| {
            format!("invalid network.mitm_hooks[{hook_index}].actions.inject_request_headers")
        })?;

        if host.is_empty() {
            return Err(anyhow!(
                "network.mitm_hooks[{hook_index}].host must not be empty"
            ));
        }
    }

    Ok(())
}

pub(crate) fn compile_mitm_hooks(config: &NetworkProxyConfig) -> Result<MitmHooksByHost> {
    compile_mitm_hooks_with_resolvers(
        config,
        |name| env::var(name).ok(),
        |path| {
            let value = fs::read_to_string(path.as_path()).with_context(|| {
                format!("failed to read secret file {}", path.as_path().display())
            })?;
            Ok(value.trim().to_string())
        },
    )
}

pub(crate) fn evaluate_mitm_hooks(
    hooks_by_host: &MitmHooksByHost,
    host: &str,
    req: &Request,
) -> HookEvaluation {
    let normalized_host = normalize_host(host);
    let Some(hooks) = hooks_by_host.get(&normalized_host) else {
        return HookEvaluation::NoHooksForHost;
    };

    for hook in hooks {
        if hook_matches(hook, req) {
            return HookEvaluation::Matched {
                actions: hook.actions.clone(),
            };
        }
    }

    HookEvaluation::HookedHostNoMatch
}

fn compile_mitm_hooks_with_resolvers<EnvFn, FileFn>(
    config: &NetworkProxyConfig,
    resolve_env_var: EnvFn,
    read_secret_file: FileFn,
) -> Result<MitmHooksByHost>
where
    EnvFn: Fn(&str) -> Option<String>,
    FileFn: Fn(&AbsolutePathBuf) -> Result<String>,
{
    validate_mitm_hook_config(config)?;

    let mut hooks_by_host = MitmHooksByHost::new();
    for hook in &config.network.mitm_hooks {
        let host = normalize_hook_host(&hook.host)?;
        let methods = normalize_methods(&hook.matcher.methods)?;
        let path_prefixes = compile_path_matchers(&hook.matcher.path_prefixes)?;
        let query = hook
            .matcher
            .query
            .iter()
            .map(|(name, values)| {
                Ok(QueryConstraint {
                    name: normalize_query_name(name)?,
                    allowed_values: compile_value_matchers(values)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let headers = hook
            .matcher
            .headers
            .iter()
            .map(|(name, values)| {
                Ok(HeaderConstraint {
                    name: parse_header_name(name)?,
                    allowed_values: compile_value_matchers(values)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let strip_request_headers = hook
            .actions
            .strip_request_headers
            .iter()
            .map(|name| parse_header_name(name))
            .collect::<Result<Vec<_>>>()?;
        let inject_request_headers = hook
            .actions
            .inject_request_headers
            .iter()
            .map(|header| {
                compile_injected_header(header, &resolve_env_var, &read_secret_file)
                    .with_context(|| format!("failed to compile injected header {}", header.name))
            })
            .collect::<Result<Vec<_>>>()?;

        hooks_by_host
            .entry(host.clone())
            .or_default()
            .push(MitmHook {
                host,
                matcher: MitmHookMatcher {
                    methods,
                    path_prefixes,
                    query,
                    headers,
                    body: None,
                },
                actions: MitmHookActions {
                    strip_request_headers,
                    inject_request_headers,
                },
            });
    }

    Ok(hooks_by_host)
}

fn compile_injected_header<EnvFn, FileFn>(
    header: &InjectedHeaderConfig,
    resolve_env_var: &EnvFn,
    read_secret_file: &FileFn,
) -> Result<ResolvedInjectedHeader>
where
    EnvFn: Fn(&str) -> Option<String>,
    FileFn: Fn(&AbsolutePathBuf) -> Result<String>,
{
    let name = parse_header_name(&header.name)?;
    let (secret, source) = match (
        header.secret_env_var.as_deref(),
        header.secret_file.as_deref(),
    ) {
        (Some(env_var), None) => {
            let value = resolve_env_var(env_var)
                .ok_or_else(|| anyhow!("missing required environment variable {env_var}"))?;
            (value, SecretSource::EnvVar(env_var.to_string()))
        }
        (None, Some(secret_file)) => {
            let path = parse_secret_file(secret_file)?;
            let value = read_secret_file(&path)?;
            (value, SecretSource::File(path))
        }
        _ => {
            return Err(anyhow!(
                "expected exactly one of secret_env_var or secret_file"
            ));
        }
    };

    let prefix = header.prefix.clone().unwrap_or_default();
    let value = HeaderValue::from_str(&format!("{prefix}{secret}"))
        .with_context(|| format!("invalid value for injected header {}", header.name))?;

    Ok(ResolvedInjectedHeader {
        name,
        value,
        source,
    })
}

fn hook_matches(hook: &MitmHook, req: &Request) -> bool {
    let method = req.method().as_str().to_ascii_uppercase();
    if !hook
        .matcher
        .methods
        .iter()
        .any(|allowed| allowed == &method)
    {
        return false;
    }

    let path = req.uri().path();
    if !path_matches(&hook.matcher.path_prefixes, path) {
        return false;
    }

    if !query_matches(&hook.matcher.query, req) {
        return false;
    }

    headers_match(&hook.matcher.headers, req)
}

fn query_matches(query_constraints: &[QueryConstraint], req: &Request) -> bool {
    if query_constraints.is_empty() {
        return true;
    }

    let actual_query = req.uri().query().unwrap_or_default();
    let mut actual_values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, value) in form_urlencoded::parse(actual_query.as_bytes()) {
        actual_values
            .entry(name.into_owned())
            .or_default()
            .push(value.into_owned());
    }

    query_constraints.iter().all(|constraint| {
        actual_values.get(&constraint.name).is_some_and(|actual| {
            actual.iter().any(|candidate| {
                constraint
                    .allowed_values
                    .iter()
                    .any(|allowed| allowed.matches(candidate))
            })
        })
    })
}

fn headers_match(header_constraints: &[HeaderConstraint], req: &Request) -> bool {
    header_constraints.iter().all(|constraint| {
        let actual = req.headers().get_all(&constraint.name);
        if actual.iter().next().is_none() {
            return false;
        }
        if constraint.allowed_values.is_empty() {
            return true;
        }

        actual.iter().any(|value| {
            value.to_str().ok().is_some_and(|candidate| {
                constraint
                    .allowed_values
                    .iter()
                    .any(|allowed| allowed.matches(candidate))
            })
        })
    })
}

fn path_matches(path_prefixes: &[PathMatcher], path: &str) -> bool {
    path_prefixes.iter().any(|matcher| matcher.matches(path))
}

impl PathMatcher {
    fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Prefix(prefix) => candidate.starts_with(prefix),
            Self::Glob(glob) => glob.is_match(candidate),
        }
    }
}

impl ValueMatcher {
    fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Exact(value) => value == candidate,
            Self::Glob(glob) => glob.is_match(candidate),
        }
    }
}

fn compile_path_matchers(path_prefixes: &[String]) -> Result<Vec<PathMatcher>> {
    path_prefixes
        .iter()
        .map(|prefix| {
            match parse_matcher_pattern(prefix)? {
                MatcherPattern::Literal(prefix) => {
                    if prefix.is_empty() {
                        return Err(anyhow!("path_prefixes must not contain empty entries"));
                    }
                    Ok(PathMatcher::Prefix(prefix.to_string()))
                }
                MatcherPattern::Glob(glob_pattern) => Ok(PathMatcher::Glob(compile_glob_matcher(
                    glob_pattern,
                    /*literal_separator*/ true,
                )?)),
            }
        })
        .collect()
}

fn compile_value_matchers(values: &[String]) -> Result<Vec<ValueMatcher>> {
    values
        .iter()
        .map(|value| match parse_matcher_pattern(value)? {
            MatcherPattern::Literal(value) => Ok(ValueMatcher::Exact(value.to_string())),
            MatcherPattern::Glob(glob_pattern) => Ok(ValueMatcher::Glob(compile_glob_matcher(
                glob_pattern,
                /*literal_separator*/ false,
            )?)),
        })
        .collect()
}

fn parse_matcher_pattern(pattern: &str) -> Result<MatcherPattern<'_>> {
    if let Some(literal) = pattern.strip_prefix(LITERAL_PREFIX) {
        return Ok(MatcherPattern::Literal(literal));
    }
    let Some(glob_pattern) = pattern.strip_prefix(PATTERN_PREFIX) else {
        return Ok(MatcherPattern::Literal(pattern));
    };
    if glob_pattern.is_empty() {
        return Err(anyhow!("glob pattern must not be empty"));
    }
    Ok(MatcherPattern::Glob(glob_pattern))
}

fn compile_glob_matcher(pattern: &str, literal_separator: bool) -> Result<CompiledGlobMatcher> {
    let mut builder = GlobBuilder::new(pattern);
    builder
        .backslash_escape(true)
        .literal_separator(literal_separator);
    builder
        .build()
        .map(|glob| CompiledGlobMatcher {
            pattern: pattern.to_string(),
            matcher: glob.compile_matcher(),
        })
        .map_err(|err| anyhow!("invalid glob pattern {pattern:?}: {err}"))
}

fn normalize_hook_host(host: &str) -> Result<String> {
    let normalized = normalize_host(host);
    if normalized.is_empty() {
        return Err(anyhow!("host must not be empty"));
    }
    if normalized.contains('*') {
        return Err(anyhow!(
            "MITM hook hosts must be exact hosts and cannot contain wildcards"
        ));
    }
    Ok(normalized)
}

fn normalize_methods(methods: &[String]) -> Result<Vec<String>> {
    methods
        .iter()
        .map(|method| {
            let normalized = method.trim().to_ascii_uppercase();
            if normalized.is_empty() {
                return Err(anyhow!("methods must not contain empty entries"));
            }
            Ok(normalized)
        })
        .collect()
}

fn validate_query_constraints(query: &BTreeMap<String, Vec<String>>) -> Result<()> {
    for (name, values) in query {
        let normalized = normalize_query_name(name)?;
        if normalized.is_empty() {
            return Err(anyhow!("query keys must not be empty"));
        }
        if values.is_empty() {
            return Err(anyhow!(
                "query key {name:?} must list at least one allowed value"
            ));
        }
        let _ = compile_value_matchers(values)
            .with_context(|| format!("invalid matcher for query key {name:?}"))?;
    }
    Ok(())
}

fn normalize_query_name(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(anyhow!("query keys must not be empty"));
    }
    Ok(name.to_string())
}

fn validate_header_constraints(headers: &BTreeMap<String, Vec<String>>) -> Result<()> {
    for (name, values) in headers {
        let _ = parse_header_name(name)?;
        let _ = compile_value_matchers(values)
            .with_context(|| format!("invalid matcher for header {name:?}"))?;
    }
    Ok(())
}

fn validate_strip_request_headers(header_names: &[String]) -> Result<()> {
    for name in header_names {
        let _ = parse_header_name(name)?;
    }
    Ok(())
}

fn validate_injected_headers(headers: &[InjectedHeaderConfig]) -> Result<()> {
    for header in headers {
        let _ = parse_header_name(&header.name)?;
        match (
            header.secret_env_var.as_deref(),
            header.secret_file.as_deref(),
        ) {
            (Some(secret_env_var), None) => {
                if secret_env_var.trim().is_empty() {
                    return Err(anyhow!("secret_env_var must not be empty"));
                }
            }
            (None, Some(secret_file)) => {
                let _ = parse_secret_file(secret_file)?;
            }
            _ => {
                return Err(anyhow!(
                    "expected exactly one of secret_env_var or secret_file"
                ));
            }
        }
    }
    Ok(())
}

fn parse_header_name(name: &str) -> Result<HeaderName> {
    HeaderName::from_bytes(name.as_bytes())
        .map_err(|err| anyhow!("invalid header name {name:?}: {err}"))
}

fn parse_secret_file(path: &str) -> Result<AbsolutePathBuf> {
    if path.trim().is_empty() {
        return Err(anyhow!("secret_file must not be empty"));
    }
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err(anyhow!("secret_file must be an absolute path: {path:?}"));
    }
    AbsolutePathBuf::from_absolute_path(path)
        .with_context(|| format!("secret_file must be an absolute path: {path:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NetworkMode;
    use crate::config::NetworkProxySettings;
    use pretty_assertions::assert_eq;
    use rama_http::Body;
    use rama_http::Method;
    use tempfile::NamedTempFile;

    fn base_config() -> NetworkProxyConfig {
        NetworkProxyConfig {
            network: NetworkProxySettings {
                mitm: true,
                mode: NetworkMode::Limited,
                ..NetworkProxySettings::default()
            },
        }
    }

    fn github_hook() -> MitmHookConfig {
        MitmHookConfig {
            host: "api.github.com".to_string(),
            matcher: MitmHookMatchConfig {
                methods: vec!["POST".to_string(), "PUT".to_string()],
                path_prefixes: vec!["/repos/openai/".to_string()],
                ..MitmHookMatchConfig::default()
            },
            actions: MitmHookActionsConfig {
                strip_request_headers: vec!["authorization".to_string()],
                inject_request_headers: vec![InjectedHeaderConfig {
                    name: "authorization".to_string(),
                    secret_env_var: Some("CODEX_GITHUB_TOKEN".to_string()),
                    secret_file: None,
                    prefix: Some("Bearer ".to_string()),
                }],
            },
        }
    }

    #[test]
    fn validate_requires_mitm_for_hooks() {
        let mut config = base_config();
        config.network.mitm = false;
        config.network.mitm_hooks = vec![github_hook()];

        let err = validate_mitm_hook_config(&config).expect_err("hooks require mitm");
        assert!(
            err.to_string()
                .contains("network.mitm_hooks requires network.mitm = true")
        );
    }

    #[test]
    fn validate_allows_hooks_in_full_mode() {
        let mut config = base_config();
        config.network.mode = NetworkMode::Full;
        config.network.mitm_hooks = vec![github_hook()];

        validate_mitm_hook_config(&config).expect("hooks should be allowed in full mode");
    }

    #[test]
    fn validate_rejects_body_matchers_for_now() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.body = Some(MitmHookBodyConfig(serde_json::json!({
            "repository": "openai/codex"
        })));
        config.network.mitm_hooks = vec![hook];

        let err = validate_mitm_hook_config(&config).expect_err("body matchers are reserved");
        assert!(err.to_string().contains("match.body is reserved"));
    }

    #[test]
    fn validate_rejects_relative_secret_file() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.actions.inject_request_headers[0].secret_env_var = None;
        hook.actions.inject_request_headers[0].secret_file = Some("token.txt".to_string());
        config.network.mitm_hooks = vec![hook];

        let err = validate_mitm_hook_config(&config).expect_err("secret file must be absolute");
        assert!(format!("{err:#}").contains("secret_file must be an absolute path"));
    }

    #[test]
    fn validate_rejects_dual_secret_sources() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.actions.inject_request_headers[0].secret_file = Some("/tmp/github-token".to_string());
        config.network.mitm_hooks = vec![hook];

        let err = validate_mitm_hook_config(&config).expect_err("dual secret sources invalid");
        assert!(format!("{err:#}").contains("exactly one of secret_env_var or secret_file"));
    }

    #[test]
    fn compile_resolves_env_backed_injected_headers() {
        let mut config = base_config();
        config.network.mitm_hooks = vec![github_hook()];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |name| (name == "CODEX_GITHUB_TOKEN").then(|| "ghp-secret".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();

        let compiled = hooks.get("api.github.com").unwrap();
        assert_eq!(compiled.len(), 1);
        assert_eq!(
            compiled[0].actions.inject_request_headers[0].source,
            SecretSource::EnvVar("CODEX_GITHUB_TOKEN".to_string())
        );
        assert_eq!(
            compiled[0].actions.inject_request_headers[0].value,
            HeaderValue::from_static("Bearer ghp-secret")
        );
    }

    #[test]
    fn compile_resolves_file_backed_injected_headers() {
        let secret_file = NamedTempFile::new().unwrap();
        std::fs::write(secret_file.path(), "ghp-file-secret\n").unwrap();

        let mut config = base_config();
        let mut hook = github_hook();
        hook.actions.inject_request_headers[0].secret_env_var = None;
        hook.actions.inject_request_headers[0].secret_file =
            Some(secret_file.path().display().to_string());
        config.network.mitm_hooks = vec![hook];

        let hooks = compile_mitm_hooks(&config).unwrap();
        let compiled = hooks.get("api.github.com").unwrap();
        assert_eq!(
            compiled[0].actions.inject_request_headers[0].value,
            HeaderValue::from_static("Bearer ghp-file-secret")
        );
    }

    #[test]
    fn evaluate_returns_first_matching_hook() {
        let mut config = base_config();
        let mut first = github_hook();
        first.matcher.path_prefixes = vec!["/repos/openai/".to_string()];
        let mut second = github_hook();
        second.actions.inject_request_headers[0].prefix = Some("Token ".to_string());
        config.network.mitm_hooks = vec![first, second];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |_| Some("abc".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();
        let req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/codex/issues")
            .header("x-trace", "1")
            .body(Body::empty())
            .unwrap();

        let evaluation = evaluate_mitm_hooks(&hooks, "api.github.com", &req);
        let HookEvaluation::Matched { actions } = evaluation else {
            panic!("expected a matching hook");
        };

        assert_eq!(
            actions.inject_request_headers[0].value,
            HeaderValue::from_static("Bearer abc")
        );
    }

    #[test]
    fn evaluate_matches_query_and_header_constraints() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.query = BTreeMap::from([(
            "state".to_string(),
            vec!["open".to_string(), "triage".to_string()],
        )]);
        hook.matcher.headers = BTreeMap::from([(
            "x-github-api-version".to_string(),
            vec!["2022-11-28".to_string()],
        )]);
        config.network.mitm_hooks = vec![hook];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |_| Some("abc".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();
        let req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/codex/issues?state=open&per_page=10")
            .header("x-github-api-version", "2022-11-28")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &req),
            HookEvaluation::Matched {
                actions: hooks.get("api.github.com").unwrap()[0].actions.clone(),
            }
        );
    }

    #[test]
    fn evaluate_matches_wildcard_path_query_and_header_constraints() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.path_prefixes = vec!["pattern:/repos/*/codex/issues*".to_string()];
        hook.matcher.query =
            BTreeMap::from([("state".to_string(), vec!["pattern:op*".to_string()])]);
        hook.matcher.headers = BTreeMap::from([(
            "x-github-api-version".to_string(),
            vec!["pattern:2022*preview".to_string()],
        )]);
        config.network.mitm_hooks = vec![hook];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |_| Some("abc".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();
        let req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/codex/issues?state=open")
            .header("x-github-api-version", "2022-11-28-preview")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &req),
            HookEvaluation::Matched {
                actions: hooks.get("api.github.com").unwrap()[0].actions.clone(),
            }
        );
    }

    #[test]
    fn validate_rejects_invalid_wildcard_path_pattern() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.path_prefixes = vec!["pattern:/repos/[".to_string()];
        config.network.mitm_hooks = vec![hook];

        let err = validate_mitm_hook_config(&config).expect_err("invalid glob should fail");
        assert!(format!("{err:#}").contains("invalid glob pattern"));
    }

    #[test]
    fn evaluate_path_wildcard_does_not_cross_segment_boundaries() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.path_prefixes = vec!["pattern:/repos/*/codex/issues*".to_string()];
        config.network.mitm_hooks = vec![hook];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |_| Some("abc".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();
        let nested_req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/private/codex/issues")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &nested_req),
            HookEvaluation::HookedHostNoMatch
        );
    }

    #[test]
    fn evaluate_treats_glob_metacharacters_as_literal_without_glob_prefix() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.path_prefixes = vec!["/repos/[draft]/".to_string()];
        hook.matcher.query = BTreeMap::from([("state".to_string(), vec!["op*".to_string()])]);
        hook.matcher.headers = BTreeMap::from([(
            "x-github-api-version".to_string(),
            vec!["2022-11-28[preview]".to_string()],
        )]);
        config.network.mitm_hooks = vec![hook];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |_| Some("abc".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();
        let exact_req = Request::builder()
            .method(Method::POST)
            .uri("/repos/[draft]/codex/issues?state=op*")
            .header("x-github-api-version", "2022-11-28[preview]")
            .body(Body::empty())
            .unwrap();
        let non_literal_req = Request::builder()
            .method(Method::POST)
            .uri("/repos/draft/codex/issues?state=open")
            .header("x-github-api-version", "2022-11-28-preview")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &exact_req),
            HookEvaluation::Matched {
                actions: hooks.get("api.github.com").unwrap()[0].actions.clone(),
            }
        );
        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &non_literal_req),
            HookEvaluation::HookedHostNoMatch
        );
    }

    #[test]
    fn evaluate_allows_literal_values_with_reserved_prefixes() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.query =
            BTreeMap::from([("state".to_string(), vec!["literal:pattern:*".to_string()])]);
        hook.matcher.headers = BTreeMap::from([(
            "x-github-api-version".to_string(),
            vec!["literal:pattern:*".to_string()],
        )]);
        config.network.mitm_hooks = vec![hook];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |_| Some("abc".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();
        let exact_req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/codex/issues?state=pattern%3A%2A")
            .header("x-github-api-version", "pattern:*")
            .body(Body::empty())
            .unwrap();
        let non_literal_req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/codex/issues?state=pattern%3Aopen")
            .header("x-github-api-version", "pattern:preview")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &exact_req),
            HookEvaluation::Matched {
                actions: hooks.get("api.github.com").unwrap()[0].actions.clone(),
            }
        );
        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &non_literal_req),
            HookEvaluation::HookedHostNoMatch
        );
    }

    #[test]
    fn evaluate_returns_hooked_host_no_match_when_query_constraint_fails() {
        let mut config = base_config();
        let mut hook = github_hook();
        hook.matcher.query = BTreeMap::from([("state".to_string(), vec!["open".to_string()])]);
        config.network.mitm_hooks = vec![hook];

        let hooks = compile_mitm_hooks_with_resolvers(
            &config,
            |_| Some("abc".to_string()),
            |_| Err(anyhow!("unexpected file lookup")),
        )
        .unwrap();
        let req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/codex/issues?state=closed")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            evaluate_mitm_hooks(&hooks, "api.github.com", &req),
            HookEvaluation::HookedHostNoMatch
        );
    }

    #[test]
    fn evaluate_returns_no_hooks_for_unconfigured_host() {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/repos/openai/codex/issues")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            evaluate_mitm_hooks(&MitmHooksByHost::new(), "api.github.com", &req),
            HookEvaluation::NoHooksForHost
        );
    }
}
