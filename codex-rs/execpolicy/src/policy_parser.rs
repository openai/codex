#![allow(clippy::needless_lifetimes)]

use crate::Opt;
use crate::Policy;
use crate::ProgramSpec;
use crate::arg_matcher::ArgMatcher;
use crate::opt::OptMeta;
use log::info;
use multimap::MultiMap;
use regex::Regex;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::LibraryExtension;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::Heap;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneType;
use std::cell::RefCell;
use std::collections::HashMap;

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

    pub fn parse(&self) -> starlark::Result<Policy> {
        let mut dialect = Dialect::Extended.clone();
        dialect.enable_f_strings = true;
        let ast = AstModule::parse(&self.policy_source, self.unparsed_policy.clone(), &dialect)?;
        let globals = GlobalsBuilder::extended_by(&[LibraryExtension::Typing])
            .with(policy_builtins)
            .build();
        let module = Module::new();

        let heap = Heap::new();

        module.set("ARG_OPAQUE_VALUE", heap.alloc(ArgMatcher::OpaqueNonFile));
        module.set("ARG_RFILE", heap.alloc(ArgMatcher::ReadableFile));
        module.set("ARG_WFILE", heap.alloc(ArgMatcher::WriteableFile));
        module.set("ARG_RFILES", heap.alloc(ArgMatcher::ReadableFiles));
        module.set(
            "ARG_RFILES_OR_CWD",
            heap.alloc(ArgMatcher::ReadableFilesOrCwd),
        );
        module.set("ARG_POS_INT", heap.alloc(ArgMatcher::PositiveInteger));
        module.set("ARG_SED_COMMAND", heap.alloc(ArgMatcher::SedCommand));
        module.set(
            "ARG_UNVERIFIED_VARARGS",
            heap.alloc(ArgMatcher::UnverifiedVarargs),
        );

        let policy_builder = PolicyBuilder::new();
        {
            let mut eval = Evaluator::new(&module);
            eval.extra = Some(&policy_builder);
            eval.eval_module(ast, &globals)?;
        }
        let policy = policy_builder.build();
        policy.map_err(|e| starlark::Error::new_kind(starlark::ErrorKind::Other(e.into())))
    }
}

#[derive(Debug)]
pub struct ForbiddenProgramRegex {
    pub regex: regex::Regex,
    pub reason: String,
}

#[derive(Debug, ProvidesStaticType)]
struct PolicyBuilder {
    programs: RefCell<MultiMap<String, ProgramSpec>>,
    forbidden_program_regexes: RefCell<Vec<ForbiddenProgramRegex>>,
    forbidden_substrings: RefCell<Vec<String>>,
}

impl PolicyBuilder {
    fn new() -> Self {
        Self {
            programs: RefCell::new(MultiMap::new()),
            forbidden_program_regexes: RefCell::new(Vec::new()),
            forbidden_substrings: RefCell::new(Vec::new()),
        }
    }

    fn build(self) -> Result<Policy, regex::Error> {
        let programs = self.programs.into_inner();
        let forbidden_program_regexes = self.forbidden_program_regexes.into_inner();
        let forbidden_substrings = self.forbidden_substrings.into_inner();
        Policy::new(programs, forbidden_program_regexes, forbidden_substrings)
    }

    fn add_program_spec(&self, program_spec: ProgramSpec) {
        info!("adding program spec: {:?}", program_spec);
        let name = program_spec.program.clone();
        let mut programs = self.programs.borrow_mut();
        programs.insert(name.clone(), program_spec);
    }

    fn add_forbidden_substrings(&self, substrings: &[String]) {
        let mut forbidden_substrings = self.forbidden_substrings.borrow_mut();
        forbidden_substrings.extend_from_slice(substrings);
    }

    fn add_forbidden_program_regex(&self, regex: Regex, reason: String) {
        let mut forbidden_program_regexes = self.forbidden_program_regexes.borrow_mut();
        forbidden_program_regexes.push(ForbiddenProgramRegex { regex, reason });
    }
}

#[starlark_module]
fn policy_builtins(builder: &mut GlobalsBuilder) {
    fn define_program<'v>(
        program: String,
        system_path: Option<UnpackList<String>>,
        option_bundling: Option<bool>,
        combined_format: Option<bool>,
        options: Option<UnpackList<Opt>>,
        args: Option<UnpackList<ArgMatcher>>,
        forbidden: Option<String>,
        should_match: Option<UnpackList<UnpackList<String>>>,
        should_not_match: Option<UnpackList<UnpackList<String>>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let option_bundling = option_bundling.unwrap_or(false);
        let system_path = system_path.map_or_else(Vec::new, |v| v.items.to_vec());
        let combined_format = combined_format.unwrap_or(false);
        let options = options.map_or_else(Vec::new, |v| v.items.to_vec());
        let args = args.map_or_else(Vec::new, |v| v.items.to_vec());

        let mut allowed_options = HashMap::<String, Opt>::new();
        for opt in options {
            let name = opt.name().to_string();
            if allowed_options
                .insert(opt.name().to_string(), opt)
                .is_some()
            {
                return Err(anyhow::format_err!("duplicate flag: {name}"));
            }
        }

        let program_spec = ProgramSpec::new(
            program,
            system_path,
            option_bundling,
            combined_format,
            allowed_options,
            args,
            forbidden,
            should_match
                .map_or_else(Vec::new, |v| v.items.to_vec())
                .into_iter()
                .map(|v| v.items.to_vec())
                .collect(),
            should_not_match
                .map_or_else(Vec::new, |v| v.items.to_vec())
                .into_iter()
                .map(|v| v.items.to_vec())
                .collect(),
        );
        let policy_builder = eval
            .extra
            .as_ref()
            .unwrap()
            .downcast_ref::<PolicyBuilder>()
            .unwrap();
        policy_builder.add_program_spec(program_spec);
        Ok(NoneType)
    }

    fn forbid_substrings(
        strings: UnpackList<String>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let policy_builder = eval
            .extra
            .as_ref()
            .unwrap()
            .downcast_ref::<PolicyBuilder>()
            .unwrap();
        policy_builder.add_forbidden_substrings(&strings.items.to_vec());
        Ok(NoneType)
    }

    fn forbid_program_regex(
        regex: String,
        reason: String,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let policy_builder = eval
            .extra
            .as_ref()
            .unwrap()
            .downcast_ref::<PolicyBuilder>()
            .unwrap();
        let compiled_regex = regex::Regex::new(&regex)?;
        policy_builder.add_forbidden_program_regex(compiled_regex, reason);
        Ok(NoneType)
    }

    fn opt(name: String, r#type: ArgMatcher, required: Option<bool>) -> anyhow::Result<Opt> {
        Ok(Opt::new(
            name,
            OptMeta::Value(r#type.arg_type()),
            required.unwrap_or(false),
        ))
    }

    fn flag(name: String) -> anyhow::Result<Opt> {
        Ok(Opt::new(name, OptMeta::Flag, false))
    }
}
