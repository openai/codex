use multimap::MultiMap;
use parking_lot::Mutex;
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
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use crate::rule::PatternToken;
use crate::rule::PrefixPattern;
use crate::rule::Rule;

pub struct PolicyParser {
    policy_source: String,
    unparsed_policy: String,
}

impl PolicyParser {
    pub fn new(policy_source: &str, unparsed_policy: &str) -> Self {
        Self {
            policy_source: policy_source.to_string(),
            unparsed_policy: unparsed_policy.to_string(),
        }
    }

    pub fn parse(&self) -> Result<crate::policy::Policy> {
        let mut dialect = Dialect::Extended.clone();
        dialect.enable_f_strings = true;
        let ast = AstModule::parse(&self.policy_source, self.unparsed_policy.clone(), &dialect)
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
    next_auto_id: AtomicU64,
}

impl PolicyBuilder {
    fn new() -> Self {
        Self {
            rules_by_program: Mutex::new(MultiMap::new()),
            next_auto_id: AtomicU64::new(0),
        }
    }

    fn alloc_id(&self) -> String {
        let id = self.next_auto_id.fetch_add(1, Ordering::Relaxed);
        format!("rule_{id}")
    }

    fn add_rule(&self, rule: Rule) {
        self.rules_by_program
            .lock()
            .insert(rule.pattern.first.clone(), rule);
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
        return Ok(PatternToken::Single(s.to_string()));
    }

    if let Some(list) = ListRef::from_value(value) {
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
        Err(Error::InvalidPattern(
            "pattern element must be a string or list of strings".to_string(),
        ))
    }
}

fn parse_examples<'v>(examples: UnpackList<Value<'v>>) -> Result<Vec<Vec<String>>> {
    examples
        .items
        .into_iter()
        .map(|example| {
            let list = ListRef::from_value(example).ok_or_else(|| {
                Error::InvalidExample("example must be a list of strings".to_string())
            })?;
            let tokens: Vec<String> = list
                .content()
                .iter()
                .map(|value| {
                    value
                        .unpack_str()
                        .ok_or_else(|| {
                            Error::InvalidExample("example tokens must be strings".to_string())
                        })
                        .map(str::to_string)
                })
                .collect::<Result<_>>()?;

            match tokens.as_slice() {
                [] => Err(Error::InvalidExample(
                    "example cannot be an empty list".to_string(),
                )),
                _ => Ok(tokens),
            }
        })
        .collect()
}

#[starlark_module]
fn policy_builtins(builder: &mut GlobalsBuilder) {
    fn prefix_rule<'v>(
        pattern: UnpackList<Value<'v>>,
        decision: Option<&'v str>,
        r#match: Option<UnpackList<Value<'v>>>,
        not_match: Option<UnpackList<Value<'v>>>,
        id: Option<&'v str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let decision = match decision {
            Some(raw) => Decision::parse(raw)?,
            None => Decision::Allow,
        };

        let pattern_tokens = parse_pattern(pattern)?;

        let positive_examples: Vec<Vec<String>> =
            r#match.map(parse_examples).transpose()?.unwrap_or_default();
        let negative_examples: Vec<Vec<String>> = not_match
            .map(parse_examples)
            .transpose()?
            .unwrap_or_default();

        let id = id.map(std::string::ToString::to_string).unwrap_or_else(|| {
            #[expect(clippy::unwrap_used)]
            let builder = eval
                .extra
                .as_ref()
                .unwrap()
                .downcast_ref::<PolicyBuilder>()
                .unwrap();
            builder.alloc_id()
        });

        let (first_token, remaining_tokens) = pattern_tokens
            .split_first()
            .ok_or_else(|| Error::InvalidPattern("pattern cannot be empty".to_string()))?;

        #[expect(clippy::unwrap_used)]
        let builder = eval
            .extra
            .as_ref()
            .unwrap()
            .downcast_ref::<PolicyBuilder>()
            .unwrap();

        for head in first_token.alternatives() {
            let rule = Rule {
                id: id.clone(),
                pattern: PrefixPattern {
                    first: head,
                    rest: remaining_tokens.to_vec(),
                },
                decision,
            };
            rule.validate_examples(&positive_examples, &negative_examples)?;
            builder.add_rule(rule);
        }
        Ok(NoneType)
    }
}
