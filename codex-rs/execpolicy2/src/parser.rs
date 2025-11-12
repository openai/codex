use multimap::MultiMap;
use parking_lot::Mutex;
use shlex;
use starlark::any::ProvidesStaticType;
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
use std::sync::Arc;

use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use crate::policy::validate_match_examples;
use crate::rule::PatternToken;
use crate::rule::PrefixPattern;
use crate::rule::PrefixRule;
use crate::rule::Rule;

// todo: support parsing multiple policies
pub struct PolicyParser;

impl PolicyParser {
    pub fn parse(policy_source: &str, unparsed_policy: &str) -> Result<crate::policy::Policy> {
        let mut dialect = Dialect::Extended.clone();
        dialect.enable_f_strings = true;
        let ast = AstModule::parse(policy_source, unparsed_policy.to_string(), &dialect)
            .map_err(|e| Error::Starlark(e.to_string()))?;
        let globals = GlobalsBuilder::standard().with(policy_builtins).build();
        let module = Module::new();

        let builder = PolicyBuilder::new();
        {
            let mut eval = Evaluator::new(&module);
            eval.extra = Some(&builder);
            eval.eval_module(ast, &globals)
                .map_err(|e| Error::Starlark(e.to_string()))?;
        }
        Ok(builder.build())
    }
}

#[derive(Debug, ProvidesStaticType)]
struct PolicyBuilder {
    rules_by_program: Mutex<MultiMap<String, Rule>>,
}

impl PolicyBuilder {
    fn new() -> Self {
        Self {
            rules_by_program: Mutex::new(MultiMap::new()),
        }
    }

    fn add_rule(&self, rule: Rule) {
        self.rules_by_program
            .lock()
            .insert(rule.program().to_string(), rule);
    }

    fn build(&self) -> crate::policy::Policy {
        crate::policy::Policy::new(self.rules_by_program.lock().clone())
    }
}

fn parse_pattern<'v>(pattern: UnpackList<Value<'v>>) -> Result<Vec<PatternToken>> {
    let tokens: Vec<PatternToken> = pattern
        .items
        .into_iter()
        .map(parse_pattern_token)
        .collect::<Result<_>>()?;
    if tokens.is_empty() {
        return Err(Error::InvalidPattern("pattern cannot be empty".to_string()));
    }

    Ok(tokens)
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
                        Error::InvalidPattern("pattern alternative must be a string".to_string())
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

    match tokens.as_slice() {
        [] => Err(Error::InvalidExample(
            "example cannot be an empty string".to_string(),
        )),
        _ => Ok(tokens),
    }
}

fn parse_list_example(list: &ListRef) -> Result<Vec<String>> {
    let tokens: Vec<String> = list
        .content()
        .iter()
        .map(|value| {
            value
                .unpack_str()
                .ok_or_else(|| Error::InvalidExample("example tokens must be strings".to_string()))
                .map(str::to_string)
        })
        .collect::<Result<_>>()?;

    match tokens.as_slice() {
        [] => Err(Error::InvalidExample(
            "example cannot be an empty list".to_string(),
        )),
        _ => Ok(tokens),
    }
}

fn policy_builder<'v, 'a>(eval: &Evaluator<'v, 'a, '_>) -> &'a PolicyBuilder {
    #[expect(clippy::unwrap_used)]
    eval.extra
        .as_ref()
        .unwrap()
        .downcast_ref::<PolicyBuilder>()
        .unwrap()
}

#[starlark_module]
fn policy_builtins(builder: &mut GlobalsBuilder) {
    fn prefix_rule<'v>(
        pattern: UnpackList<Value<'v>>,
        decision: Option<&'v str>,
        r#match: Option<UnpackList<Value<'v>>>,
        not_match: Option<UnpackList<Value<'v>>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let decision = match decision {
            Some(raw) => Decision::parse(raw)?,
            None => Decision::Allow,
        };

        let pattern_tokens = parse_pattern(pattern)?;

        let matches: Vec<Vec<String>> =
            r#match.map(parse_examples).transpose()?.unwrap_or_default();
        let not_matches: Vec<Vec<String>> = not_match
            .map(parse_examples)
            .transpose()?
            .unwrap_or_default();

        let builder = policy_builder(eval);

        let (first_token, remaining_tokens) = pattern_tokens
            .split_first()
            .ok_or_else(|| Error::InvalidPattern("pattern cannot be empty".to_string()))?;

        let rest: Arc<[PatternToken]> = remaining_tokens.to_vec().into();

        let rules: Vec<Rule> = first_token
            .alternatives()
            .iter()
            .map(|head| {
                Rule::Prefix(PrefixRule {
                    pattern: PrefixPattern {
                        first: Arc::from(head.as_str()),
                        rest: rest.clone(),
                    },
                    decision,
                })
            })
            .collect();

        rules
            .iter()
            .try_for_each(|rule| rule.validate_not_matches(&not_matches))?;

        validate_match_examples(&rules, &matches)?;

        rules.into_iter().for_each(|rule| builder.add_rule(rule));
        Ok(NoneType)
    }
}
