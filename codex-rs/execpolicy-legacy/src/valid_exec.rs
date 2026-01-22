use crate::arg_type::ArgType;
use crate::error::Result;
use serde::Serialize;

/// exec() invocation that has been accepted by a `Policy`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ValidExec {
    pub program: String,
    pub flags: Vec<MatchedFlag>,
    pub opts: Vec<MatchedOpt>,
    pub args: Vec<MatchedArg>,

    /// If non-empty, a prioritized list of paths to try instead of `program`.
    /// For example, `/bin/ls` is harder to compromise than whatever `ls`
    /// happens to be in the user's `$PATH`, so `/bin/ls` would be included for
    /// `ls`. The caller is free to disregard this list and use `program`.
    pub system_path: Vec<String>,
}

impl ValidExec {
    pub fn new(program: &str, args: Vec<MatchedArg>, system_path: &[&str]) -> Self {
        Self {
            program: program.to_string(),
            flags: vec![],
            opts: vec![],
            args,
            system_path: system_path.iter().map(|&s| s.to_string()).collect(),
        }
    }

    /// Whether a possible side effect of running this command includes writing
    /// a file.
    pub fn might_write_files(&self) -> bool {
        self.opts.iter().any(|opt| opt.r#type.might_write_file())
            || self.args.iter().any(|opt| opt.r#type.might_write_file())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatchedArg {
    pub index: usize,
    pub r#type: ArgType,
    pub value: String,
}

impl MatchedArg {
    pub fn new(index: usize, r#type: ArgType, value: &str) -> Result<Self> {
        r#type.validate(value)?;
        Ok(Self {
            index,
            r#type,
            value: value.to_string(),
        })
    }
}

/// A match for an option declared with opt() in a .policy file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatchedOpt {
    /// Name of the option that was matched.
    pub name: String,
    /// Value supplied for the option.
    pub value: String,
    /// Type of the value supplied for the option.
    pub r#type: ArgType,
}

impl MatchedOpt {
    pub fn new(name: &str, value: &str, r#type: ArgType) -> Result<Self> {
        r#type.validate(value)?;
        Ok(Self {
            name: name.to_string(),
            value: value.to_string(),
            r#type,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatchedFlag {
    /// Name of the flag that was matched.
    pub name: String,
}

impl MatchedFlag {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}
