use codex_utils_absolute_path::AbsolutePathBuf;
use multimap::MultiMap;
use shlex;
use starlark::any::ProvidesStaticType;
use starlark::codemap::FileSpan;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneType;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::decision::Decision;
use crate::error::Error;
use crate::error::ErrorLocation;
use crate::error::Result;
use crate::error::TextPosition;
use crate::error::TextRange;
use crate::executable_name::executable_lookup_key;
use crate::executable_name::executable_path_lookup_key;
use crate::rule::NetworkRule;
use crate::rule::NetworkRuleProtocol;
use crate::rule::PatternToken;
use crate::rule::PrefixPattern;
use crate::rule::PrefixRule;
use crate::rule::RuleRef;
use crate::rule::validate_match_examples;
use crate::rule::validate_not_match_examples;

pub struct PolicyParser {
    builder: RefCell<PolicyBuilder>,
}

impl Default for PolicyParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyParser {
    pub fn new() -> Self {
        Self {
            builder: RefCell::new(PolicyBuilder::new()),
        }
    }

    /// Parses a policy, tagging parser errors with `policy_identifier` so failures include the
    /// identifier alongside line numbers.
    pub fn parse(&mut self, policy_identifier: &str, policy_file_contents: &str) -> Result<()> {
        let pending_validation_count = self.builder.borrow().pending_example_validations.len();
        let mut dialect = Dialect::Extended.clone();
        dialect.enable_f_strings = true;
        let ast = AstModule::parse(
            policy_identifier,
            policy_file_contents.to_string(),
            &dialect,
        )
        .map_err(Error::Starlark)?;
        let globals = GlobalsBuilder::standard().with(policy_builtins).build();
        let module = Module::new();
        {
            let mut eval = Evaluator::new(&module);
            eval.extra = Some(&self.builder);
            eval.eval_module(ast, &globals).map_err(Error::Starlark)?;
        }
        self.builder
            .borrow()
            .validate_pending_examples_from(pending_validation_count)?;
        Ok(())
    }

    pub fn build(self) -> crate::policy::Policy {
        self.builder.into_inner().build()
    }
}

#[derive(Debug, ProvidesStaticType)]
struct PolicyBuilder {
    rules_by_program: MultiMap<String, RuleRef>,
    network_rules: Vec<NetworkRule>,
    host_executables_by_name: HashMap<String, Arc<[AbsolutePathBuf]>>,
    pending_example_validations: Vec<PendingExampleValidation>,
}

impl PolicyBuilder {
    fn new() -> Self {
        Self {
            rules_by_program: MultiMap::new(),
            network_rules: Vec::new(),
            host_executables_by_name: HashMap::new(),
            pending_example_validations: Vec::new(),
        }
    }

    fn add_rule(&mut self, rule: RuleRef) {
        self.rules_by_program
            .insert(rule.program().to_string(), rule);
    }

    fn add_network_rule(&mut self, rule: NetworkRule) {
        self.network_rules.push(rule);
    }

    fn add_host_executable(&mut self, name: String, paths: Vec<AbsolutePathBuf>) {
        self.host_executables_by_name.insert(name, paths.into());
    }

    fn add_pending_example_validation(
        &mut self,
        rules: Vec<RuleRef>,
        matches: Vec<Vec<String>>,
        not_matches: Vec<Vec<String>>,
        location: Option<ErrorLocation>,
    ) {
        self.pending_example_validations
            .push(PendingExampleValidation {
                rules,
                matches,
                not_matches,
                location,
            });
    }

    fn validate_pending_examples_from(&self, start: usize) -> Result<()> {
        for validation in &self.pending_example_validations[start..] {
            let mut rules_by_program = MultiMap::new();
            for rule in &validation.rules {
                rules_by_program.insert(rule.program().to_string(), rule.clone());
            }

            let policy = crate::policy::Policy::from_parts(
                rules_by_program,
                Vec::new(),
                self.host_executables_by_name.clone(),
            );
            validate_not_match_examples(&policy, &validation.rules, &validation.not_matches)
                .map_err(|error| attach_validation_location(error, validation.location.clone()))?;
            validate_match_examples(&policy, &validation.rules, &validation.matches)
                .map_err(|error| attach_validation_location(error, validation.location.clone()))?;
        }

        Ok(())
    }

    fn build(self) -> crate::policy::Policy {
        crate::policy::Policy::from_parts(
            self.rules_by_program,
            self.network_rules,
            self.host_executables_by_name,
        )
    }
}

#[derive(Debug)]
struct PendingExampleValidation {
    rules: Vec<RuleRef>,
    matches: Vec<Vec<String>>,
    not_matches: Vec<Vec<String>>,
    location: Option<ErrorLocation>,
}

fn parse_pattern<'v>(pattern: UnpackList<Value<'v>>) -> Result<Vec<PatternToken>> {
    let tokens: Vec<PatternToken> = pattern
        .items
        .into_iter()
        .map(parse_pattern_token)
        .collect::<Result<_>>()?;
    if tokens.is_empty() {
        Err(Error::InvalidPattern("pattern cannot be empty".to_string()))
    } else {
        Ok(tokens)
    }
}

fn parse_pattern_token<'v>(value: Value<'v>) -> Result<PatternToken> {
    if let Some(s) = value.unpack_str() {
        Ok(PatternToken::Single(s.to_string()))
    } else if let Some(list) = ListRef::from_value(value) {
        let tokens: Vec<String> = list
            .content()
            .iter()
            .map(|value| {
                value
                    .unpack_str()
                    .ok_or_else(|| {
                        Error::InvalidPattern(format!(
                            "pattern alternative must be a string (got {})",
                            value.get_type()
                        ))
                    })
                    .map(str::to_string)
            })
            .collect::<Result<_>>()?;

        match tokens.as_slice() {
            [] => Err(Error::InvalidPattern(
                "pattern alternatives cannot be empty".to_string(),
            )),
            [single] => Ok(PatternToken::Single(single.clone())),
            _ => Ok(PatternToken::Alts(tokens)),
        }
    } else {
        Err(Error::InvalidPattern(format!(
            "pattern element must be a string or list of strings (got {})",
            value.get_type()
        )))
    }
}

fn parse_examples<'v>(examples: UnpackList<Value<'v>>) -> Result<Vec<Vec<String>>> {
    examples.items.into_iter().map(parse_example).collect()
}

fn parse_literal_absolute_path(raw: &str) -> Result<AbsolutePathBuf> {
    if !Path::new(raw).is_absolute() {
        return Err(Error::InvalidRule(format!(
            "host_executable paths must be absolute (got {raw})"
        )));
    }

    AbsolutePathBuf::try_from(raw.to_string())
        .map_err(|error| Error::InvalidRule(format!("invalid absolute path `{raw}`: {error}")))
}

fn validate_host_executable_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidRule(
            "host_executable name cannot be empty".to_string(),
        ));
    }

    let path = Path::new(name);
    if path.components().count() != 1
        || path.file_name().and_then(|value| value.to_str()) != Some(name)
    {
        return Err(Error::InvalidRule(format!(
            "host_executable name must be a bare executable name (got {name})"
        )));
    }

    Ok(())
}

fn parse_network_rule_decision(raw: &str) -> Result<Decision> {
    match raw {
        "deny" => Ok(Decision::Forbidden),
        other => Decision::parse(other),
    }
}

fn error_location_from_file_span(span: FileSpan) -> ErrorLocation {
    let resolved = span.resolve_span();
    ErrorLocation {
        path: span.filename().to_string(),
        range: TextRange {
            start: TextPosition {
                line: resolved.begin.line + 1,
                column: resolved.begin.column + 1,
            },
            end: TextPosition {
                line: resolved.end.line + 1,
                column: resolved.end.column + 1,
            },
        },
    }
}

fn attach_validation_location(error: Error, location: Option<ErrorLocation>) -> Error {
    match location {
        Some(location) => error.with_location(location),
        None => error,
    }
}

fn parse_example<'v>(value: Value<'v>) -> Result<Vec<String>> {
    if let Some(raw) = value.unpack_str() {
        parse_string_example(raw)
    } else if let Some(list) = ListRef::from_value(value) {
        parse_list_example(list)
    } else {
        Err(Error::InvalidExample(format!(
            "example must be a string or list of strings (got {})",
            value.get_type()
        )))
    }
}

fn parse_string_example(raw: &str) -> Result<Vec<String>> {
    let tokens = shlex::split(raw).ok_or_else(|| {
        Error::InvalidExample("example string has invalid shell syntax".to_string())
    })?;

    if tokens.is_empty() {
        Err(Error::InvalidExample(
            "example cannot be an empty string".to_string(),
        ))
    } else {
        Ok(tokens)
    }
}

fn parse_list_example(list: &ListRef) -> Result<Vec<String>> {
    let tokens: Vec<String> = list
        .content()
        .iter()
        .map(|value| {
            value
                .unpack_str()
                .ok_or_else(|| {
                    Error::InvalidExample(format!(
                        "example tokens must be strings (got {})",
                        value.get_type()
                    ))
                })
                .map(str::to_string)
        })
        .collect::<Result<_>>()?;

    if tokens.is_empty() {
        Err(Error::InvalidExample(
            "example cannot be an empty list".to_string(),
        ))
    } else {
        Ok(tokens)
    }
}

fn policy_builder<'v, 'a>(eval: &Evaluator<'v, 'a, '_>) -> RefMut<'a, PolicyBuilder> {
    #[expect(clippy::expect_used)]
    eval.extra
        .as_ref()
        .expect("policy_builder requires Evaluator.extra to be populated")
        .downcast_ref::<RefCell<PolicyBuilder>>()
        .expect("Evaluator.extra must contain a PolicyBuilder")
        .borrow_mut()
}

#[starlark_module]
fn policy_builtins(builder: &mut GlobalsBuilder) {
    fn prefix_rule<'v>(
        pattern: UnpackList<Value<'v>>,
        decision: Option<&'v str>,
        r#match: Option<UnpackList<Value<'v>>>,
        not_match: Option<UnpackList<Value<'v>>>,
        justification: Option<&'v str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let decision = match decision {
            Some(raw) => Decision::parse(raw)?,
            None => Decision::Allow,
        };

        let justification = match justification {
            Some(raw) if raw.trim().is_empty() => {
                return Err(Error::InvalidRule("justification cannot be empty".to_string()).into());
            }
            Some(raw) => Some(raw.to_string()),
            None => None,
        };

        let pattern_tokens = parse_pattern(pattern)?;

        let matches: Vec<Vec<String>> =
            r#match.map(parse_examples).transpose()?.unwrap_or_default();
        let not_matches: Vec<Vec<String>> = not_match
            .map(parse_examples)
            .transpose()?
            .unwrap_or_default();
        let location = eval
            .call_stack_top_location()
            .map(error_location_from_file_span);

        let mut builder = policy_builder(eval);

        let (first_token, remaining_tokens) = pattern_tokens
            .split_first()
            .ok_or_else(|| Error::InvalidPattern("pattern cannot be empty".to_string()))?;

        let rest: Arc<[PatternToken]> = remaining_tokens.to_vec().into();

        let rules: Vec<RuleRef> = first_token
            .alternatives()
            .iter()
            .map(|head| {
                Arc::new(PrefixRule {
                    pattern: PrefixPattern {
                        first: Arc::from(head.as_str()),
                        rest: rest.clone(),
                    },
                    decision,
                    justification: justification.clone(),
                }) as RuleRef
            })
            .collect();

        builder.add_pending_example_validation(rules.clone(), matches, not_matches, location);
        rules.into_iter().for_each(|rule| builder.add_rule(rule));
        Ok(NoneType)
    }

    fn network_rule<'v>(
        host: &'v str,
        protocol: &'v str,
        decision: &'v str,
        justification: Option<&'v str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let protocol = NetworkRuleProtocol::parse(protocol)?;
        let decision = parse_network_rule_decision(decision)?;
        let justification = match justification {
            Some(raw) if raw.trim().is_empty() => {
                return Err(Error::InvalidRule("justification cannot be empty".to_string()).into());
            }
            Some(raw) => Some(raw.to_string()),
            None => None,
        };

        let mut builder = policy_builder(eval);
        builder.add_network_rule(NetworkRule {
            host: crate::rule::normalize_network_rule_host(host)?,
            protocol,
            decision,
            justification,
        });
        Ok(NoneType)
    }

    fn host_executable<'v>(
        name: &'v str,
        paths: UnpackList<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        validate_host_executable_name(name)?;

        let mut parsed_paths = Vec::new();
        for value in paths.items {
            let raw = value.unpack_str().ok_or_else(|| {
                Error::InvalidRule(format!(
                    "host_executable paths must be strings (got {})",
                    value.get_type()
                ))
            })?;
            let path = parse_literal_absolute_path(raw)?;
            let Some(path_name) = executable_path_lookup_key(path.as_path()) else {
                return Err(Error::InvalidRule(format!(
                    "host_executable path `{raw}` must have basename `{name}`"
                ))
                .into());
            };
            if path_name != executable_lookup_key(name) {
                return Err(Error::InvalidRule(format!(
                    "host_executable path `{raw}` must have basename `{name}`"
                ))
                .into());
            }
            if !parsed_paths.iter().any(|existing| existing == &path) {
                parsed_paths.push(path);
            }
        }

        policy_builder(eval).add_host_executable(executable_lookup_key(name), parsed_paths);
        Ok(NoneType)
    }
}
