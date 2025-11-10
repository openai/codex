use std::cell::RefCell;

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

use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
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
    rules: RefCell<Vec<Rule>>,
    next_auto_id: RefCell<i64>,
}

impl PolicyBuilder {
    fn new() -> Self {
        Self {
            rules: RefCell::new(Vec::new()),
            next_auto_id: RefCell::new(0),
        }
    }

    fn alloc_id(&self) -> String {
        let mut next = self.next_auto_id.borrow_mut();
        let id = *next;
        *next += 1;
        format!("rule_{id}")
    }

    fn add_rule(&self, rule: Rule) {
        self.rules.borrow_mut().push(rule);
    }

    fn build(&self) -> crate::policy::Policy {
        crate::policy::Policy::new(self.rules.borrow().clone())
    }
}

#[derive(Debug)]
enum PatternPart {
    Single(String),
    Alts(Vec<String>),
}

fn expand_pattern(parts: &[PatternPart]) -> Vec<Vec<String>> {
    let mut acc: Vec<Vec<String>> = vec![Vec::new()];
    for part in parts {
        let alts: Vec<String> = match part {
            PatternPart::Single(s) => vec![s.clone()],
            PatternPart::Alts(v) => v.clone(),
        };
        let mut next = Vec::new();
        for prefix in &acc {
            for alt in &alts {
                let mut combined = prefix.clone();
                combined.push(alt.clone());
                next.push(combined);
            }
        }
        acc = next;
    }
    acc
}

fn parse_pattern<'v>(pattern: UnpackList<Value<'v>>) -> Result<Vec<Vec<String>>> {
    let mut parts = Vec::new();
    for item in pattern.items {
        if let Some(s) = item.unpack_str() {
            parts.push(PatternPart::Single(s.to_string()));
            continue;
        }
        let mut alts = Vec::new();
        if let Some(list) = ListRef::from_value(item) {
            for value in list.content() {
                let s = value.unpack_str().ok_or_else(|| {
                    Error::InvalidPattern("pattern alternative must be a string".to_string())
                })?;
                alts.push(s.to_string());
            }
        } else {
            return Err(Error::InvalidPattern(
                "pattern element must be a string or list of strings".to_string(),
            ));
        }
        if alts.is_empty() {
            return Err(Error::InvalidPattern(
                "pattern alternatives cannot be empty".to_string(),
            ));
        }
        parts.push(PatternPart::Alts(alts));
    }
    Ok(expand_pattern(&parts))
}

fn parse_examples<'v>(examples: UnpackList<Value<'v>>) -> Result<Vec<Vec<String>>> {
    let mut parsed = Vec::new();
    for example in examples.items {
        let list = ListRef::from_value(example).ok_or_else(|| {
            Error::InvalidExample("example must be a list of strings".to_string())
        })?;
        let mut tokens = Vec::new();
        for value in list.content() {
            let token = value.unpack_str().ok_or_else(|| {
                Error::InvalidExample("example tokens must be strings".to_string())
            })?;
            tokens.push(token.to_string());
        }
        if tokens.is_empty() {
            return Err(Error::InvalidExample(
                "example cannot be an empty list".to_string(),
            ));
        }
        parsed.push(tokens);
    }
    Ok(parsed)
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

        let prefixes = parse_pattern(pattern)?;

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

        let rule = Rule {
            id: id.clone(),
            prefixes,
            decision,
        };
        rule.validate_examples(&positive_examples, &negative_examples)?;

        #[expect(clippy::unwrap_used)]
        let builder = eval
            .extra
            .as_ref()
            .unwrap()
            .downcast_ref::<PolicyBuilder>()
            .unwrap();
        builder.add_rule(rule);
        Ok(NoneType)
    }
}
