// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Module defining the logic for compiling the deserialized json file into
//! the IR. (Intermediate Representation)
//!
//! It also defines some of the objects that a JSON seccomp filter is deserialized into:
//! [`JsonFilter`](struct.JsonFilter.html),
//! [`JsonRule`](struct.JsonRule.html),
//! [`JsonCondition`](struct.JsonCondition.html).
//
//! The rest of objects are deserialized directly into the IR :
//! [`SeccompCondition`](struct.SeccompCondition.html),
//! [`SeccompAction`](enum.SeccompAction.html),
//! [`SeccompCmpOp`](enum.SeccompCmpOp.html),
//! [`SeccompCmpArgLen`](enum.SeccompCmpArgLen.html).

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::Read;
use std::result;

use crate::backend::{
    Error as BackendError, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition,
    SeccompFilter, SeccompRule, TargetArch,
};
use crate::syscall_table::SyscallTable;
use serde::de::{self, Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

type Result<T> = result::Result<T, Error>;

/// Error compiling JSON into IR.
#[derive(Debug)]
pub enum Error {
    /// Backend error creating the `SeccompFilter` IR.
    Backend(BackendError),
    /// Error deserializing JSON.
    SerdeJson(serde_json::Error),
    /// Invalid syscall name for the given arch.
    SyscallName(String, TargetArch),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use self::Error::*;

        match self {
            Backend(error) => Some(error),
            SerdeJson(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match self {
            Backend(error) => write!(f, "{}", error),
            SerdeJson(error) => {
                write!(f, "Error parsing Json: {}", error)
            }
            SyscallName(syscall_name, arch) => write!(
                f,
                "Invalid syscall name: {} for given arch: {:?}.",
                syscall_name, arch
            ),
        }
    }
}

/// Deserializable object that represents the top-level map of Json Filters.
// Need the 'newtype' pattern so that we can implement a custom deserializer.
pub(crate) struct JsonFilterMap(pub HashMap<String, JsonFilter>);

// Implement a custom deserializer, that returns an error for duplicate thread keys.
impl<'de> Deserialize<'de> for JsonFilterMap {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct JsonFilterMapVisitor;

        impl<'d> Visitor<'d> for JsonFilterMapVisitor {
            type Value = HashMap<String, JsonFilter>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
                f.write_str("a map of filters")
            }

            fn visit_map<M>(self, mut access: M) -> result::Result<Self::Value, M::Error>
            where
                M: MapAccess<'d>,
            {
                let mut values = Self::Value::with_capacity(access.size_hint().unwrap_or(0));

                while let Some((key, value)) = access.next_entry()? {
                    if values.insert(key, value).is_some() {
                        return Err(M::Error::custom("duplicate filter key"));
                    };
                }

                Ok(values)
            }
        }
        Ok(JsonFilterMap(
            deserializer.deserialize_map(JsonFilterMapVisitor)?,
        ))
    }
}

/// Dummy placeholder type for a JSON comment. Holds no value.
/// Used for adding comments in the JSON file, since the standard does not allow for native
/// comments.
/// This type declaration is needed so that we can implement a custom deserializer for it.
#[derive(PartialEq, Debug, Clone)]
struct JsonComment;

// Implement a custom deserializer that only validates that the comment is a string and drops the
// value.
impl<'de> Deserialize<'de> for JsonComment {
    fn deserialize<D>(deserializer: D) -> std::result::Result<JsonComment, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?;

        Ok(JsonComment {})
    }
}

/// Condition that a syscall must match in order to satisfy a rule.
// Almost equivalent to the [`SeccompCondition`](struct.html.SeccompCondition), with the added
// optional json `comment` property.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonCondition {
    /// Index of the argument that is to be compared.
    #[serde(rename = "index")]
    arg_index: u8,
    /// Length of the argument value that is to be compared.
    #[serde(rename = "type")]
    arg_len: SeccompCmpArgLen,
    /// Comparison operator to perform.
    #[serde(rename = "op")]
    operator: SeccompCmpOp,
    /// The value that will be compared with the argument value of the syscall.
    #[serde(rename = "val")]
    value: u64,
    /// Optional empty value, represents a `comment` property in the JSON file.
    comment: Option<JsonComment>,
}

impl TryFrom<JsonCondition> for SeccompCondition {
    type Error = Error;

    fn try_from(json_cond: JsonCondition) -> Result<Self> {
        SeccompCondition::new(
            json_cond.arg_index,
            json_cond.arg_len,
            json_cond.operator,
            json_cond.value,
        )
        .map_err(Error::Backend)
    }
}

/// Deserializable object representing a rule associated to a syscall.
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonRule {
    /// Name of the syscall.
    syscall: String,
    /// Rule conditions.
    #[serde(rename = "args")]
    conditions: Option<Vec<JsonCondition>>,
    /// Optional empty value, represents a `comment` property in the JSON file.
    comment: Option<JsonComment>,
}

/// Deserializable seccomp filter.
#[derive(Deserialize, PartialEq, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonFilter {
    /// Default action if no rules match. e.g. `Kill` for an AllowList.
    #[serde(alias = "default_action")]
    mismatch_action: SeccompAction,
    /// Default action if a rule matches. e.g. `Allow` for an AllowList.
    #[serde(alias = "filter_action")]
    match_action: SeccompAction,
    /// The collection of `JsonRule`s.
    #[serde(rename = "filter")]
    rules: Vec<JsonRule>,
}

/// Object responsible for compiling [`JsonFilter`](struct.JsonFilter.html)s into
/// [`SeccompFilter`](../backend/struct.SeccompFilter.html)s, which represent the IR.
pub(crate) struct JsonCompiler {
    /// Target architecture. Can be different from the current `target_arch`.
    arch: TargetArch,
    /// Target-specific syscall table.
    syscall_table: SyscallTable,
}

impl JsonCompiler {
    /// Create a new `Compiler` instance, for the given target architecture.
    pub fn new(arch: TargetArch) -> Self {
        Self {
            arch,
            syscall_table: SyscallTable::new(arch),
        }
    }

    /// Main compilation function.
    // This can easily be extracted to a Frontend trait if seccompiler will need to support
    // multiple frontend types (YAML, etc.)
    pub fn compile<R: Read>(&self, reader: R) -> Result<HashMap<String, SeccompFilter>> {
        let filters: JsonFilterMap = serde_json::from_reader(reader).map_err(Error::SerdeJson)?;
        let filters = filters.0;
        let mut bpf_map: HashMap<String, SeccompFilter> = HashMap::with_capacity(filters.len());

        for (name, filter) in filters.into_iter() {
            bpf_map.insert(name, self.make_seccomp_filter(filter)?);
        }
        Ok(bpf_map)
    }

    /// Transforms the deserialized `JsonFilter` into a `SeccompFilter` (IR language).
    fn make_seccomp_filter(&self, filter: JsonFilter) -> Result<SeccompFilter> {
        let mut rule_map: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

        for json_rule in filter.rules {
            let syscall_name = json_rule.syscall;
            let syscall_nr = self
                .syscall_table
                .get_syscall_nr(&syscall_name)
                .ok_or_else(|| Error::SyscallName(syscall_name.clone(), self.arch))?;
            let rule_accumulator = rule_map.entry(syscall_nr).or_default();

            if let Some(conditions) = json_rule.conditions {
                let mut seccomp_conditions = Vec::with_capacity(conditions.len());
                for condition in conditions {
                    seccomp_conditions.push(condition.try_into()?);
                }
                rule_accumulator
                    .push(SeccompRule::new(seccomp_conditions).map_err(Error::Backend)?);
            }
        }

        SeccompFilter::new(
            rule_map,
            filter.mismatch_action,
            filter.match_action,
            self.arch,
        )
        .map_err(Error::Backend)
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, JsonCompiler, JsonCondition, JsonFilter, JsonRule};
    use crate::backend::{
        Error as BackendError, SeccompAction, SeccompCmpArgLen, SeccompCmpArgLen::*, SeccompCmpOp,
        SeccompCmpOp::*, SeccompCondition as Cond, SeccompFilter, SeccompRule,
    };
    use std::collections::HashMap;
    use std::convert::TryInto;
    use std::env::consts::ARCH;

    impl JsonFilter {
        pub fn new(
            mismatch_action: SeccompAction,
            match_action: SeccompAction,
            rules: Vec<JsonRule>,
        ) -> JsonFilter {
            JsonFilter {
                mismatch_action,
                match_action,
                rules,
            }
        }
    }

    impl JsonRule {
        pub fn new(syscall: String, conditions: Option<Vec<JsonCondition>>) -> JsonRule {
            JsonRule {
                syscall,
                conditions,
                comment: None,
            }
        }
    }

    impl JsonCondition {
        pub fn new(
            arg_index: u8,
            arg_len: SeccompCmpArgLen,
            operator: SeccompCmpOp,
            value: u64,
        ) -> Self {
            Self {
                arg_index,
                arg_len,
                operator,
                value,
                comment: None,
            }
        }
    }

    #[test]
    // Test the transformation of `JsonFilter` objects into `SeccompFilter` objects. (JSON to IR)
    fn test_make_seccomp_filter() {
        let compiler = JsonCompiler::new(ARCH.try_into().unwrap());

        // Test with malformed filters.
        let wrong_syscall_name_filter = JsonFilter::new(
            SeccompAction::Trap,
            SeccompAction::Allow,
            vec![JsonRule::new("wrong_syscall".to_string(), None)],
        );

        assert!(matches!(
            compiler
                .make_seccomp_filter(wrong_syscall_name_filter)
                .unwrap_err(),
            Error::SyscallName(_, _)
        ));

        // Test that `SeccompConditions` validations are triggered and caught by the compilation.
        let wrong_arg_index_filter = JsonFilter::new(
            SeccompAction::Allow,
            SeccompAction::Trap,
            vec![JsonRule::new(
                "futex".to_string(),
                Some(vec![JsonCondition::new(8, Dword, Le, 65)]),
            )],
        );

        assert!(matches!(
            compiler
                .make_seccomp_filter(wrong_arg_index_filter)
                .unwrap_err(),
            Error::Backend(BackendError::InvalidArgumentNumber)
        ));

        // Test that `SeccompRule` validations are triggered and caught by the compilation.
        let empty_rule_filter = JsonFilter::new(
            SeccompAction::Allow,
            SeccompAction::Trap,
            vec![JsonRule::new("read".to_string(), Some(vec![]))],
        );

        assert!(matches!(
            compiler.make_seccomp_filter(empty_rule_filter).unwrap_err(),
            Error::Backend(BackendError::EmptyRule)
        ));

        // Test that `SeccompFilter` validations are triggered and caught by the compilation.
        let wrong_syscall_name_filter = JsonFilter::new(
            SeccompAction::Allow,
            SeccompAction::Allow,
            vec![JsonRule::new("read".to_string(), None)],
        );

        assert!(matches!(
            compiler
                .make_seccomp_filter(wrong_syscall_name_filter)
                .unwrap_err(),
            Error::Backend(BackendError::IdenticalActions)
        ));

        // Test a well-formed filter.
        let filter = JsonFilter::new(
            SeccompAction::Trap,
            SeccompAction::Allow,
            vec![
                JsonRule::new("read".to_string(), None),
                JsonRule::new(
                    "futex".to_string(),
                    Some(vec![
                        JsonCondition::new(2, Dword, Le, 65),
                        JsonCondition::new(1, Qword, Ne, 80),
                    ]),
                ),
                JsonRule::new(
                    "futex".to_string(),
                    Some(vec![
                        JsonCondition::new(3, Qword, Gt, 65),
                        JsonCondition::new(1, Qword, Lt, 80),
                    ]),
                ),
                JsonRule::new(
                    "futex".to_string(),
                    Some(vec![JsonCondition::new(3, Qword, Ge, 65)]),
                ),
                JsonRule::new(
                    "ioctl".to_string(),
                    Some(vec![JsonCondition::new(3, Dword, MaskedEq(100), 65)]),
                ),
            ],
        );

        // The expected IR.
        let seccomp_filter = SeccompFilter::new(
            vec![
                (
                    compiler.syscall_table.get_syscall_nr("read").unwrap(),
                    vec![],
                ),
                (
                    compiler.syscall_table.get_syscall_nr("futex").unwrap(),
                    vec![
                        SeccompRule::new(vec![
                            Cond::new(2, Dword, Le, 65).unwrap(),
                            Cond::new(1, Qword, Ne, 80).unwrap(),
                        ])
                        .unwrap(),
                        SeccompRule::new(vec![
                            Cond::new(3, Qword, Gt, 65).unwrap(),
                            Cond::new(1, Qword, Lt, 80).unwrap(),
                        ])
                        .unwrap(),
                        SeccompRule::new(vec![Cond::new(3, Qword, Ge, 65).unwrap()]).unwrap(),
                    ],
                ),
                (
                    compiler.syscall_table.get_syscall_nr("ioctl").unwrap(),
                    vec![
                        SeccompRule::new(vec![Cond::new(3, Dword, MaskedEq(100), 65).unwrap()])
                            .unwrap(),
                    ],
                ),
            ]
            .into_iter()
            .collect(),
            SeccompAction::Trap,
            SeccompAction::Allow,
            ARCH.try_into().unwrap(),
        )
        .unwrap();

        assert_eq!(
            compiler.make_seccomp_filter(filter).unwrap(),
            seccomp_filter
        );
    }

    #[allow(clippy::useless_asref)]
    #[test]
    fn test_compile() {
        let compiler = JsonCompiler::new(ARCH.try_into().unwrap());
        // test with malformed JSON
        {
            // empty file
            let json_input = "";
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // not json
            let json_input = "hjkln";
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // top-level array
            let json_input = "[]";
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // thread key must be a string
            let json_input = "{1}";
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // empty Filter object
            let json_input = r#"{"a": {}}"#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // missing 'filter' field
            let json_input = r#"{"a": {"match_action": "allow", "mismatch_action":"log"}}"#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // wrong key 'filters'
            let json_input =
                r#"{"a": {"match_action": "allow", "mismatch_action":"log", "filters": []}}"#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // wrong action 'logs'
            let json_input =
                r#"{"a": {"match_action": "allow", "mismatch_action":"logs", "filter": []}}"#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // duplicate action fields using aliases
            let json_input = r#"{
                    "a": {
                        "match_action": "allow",
                        "mismatch_action":"log",
                        "filter_action": "trap",
                        "filter": []
                    }
                }"#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // action that expects a value
            let json_input =
                r#"{"a": {"match_action": "allow", "mismatch_action":"errno", "filter": []}}"#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // overflowing u64 value
            let json_input = r#"
            {
                "thread_2": {
                    "mismatch_action": "trap",
                    "match_action": "allow",
                    "filter": [
                        {
                            "syscall": "ioctl",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "qword",
                                    "op": "eq",
                                    "val": 18446744073709551616
                                }
                            ]
                        }
                    ]
                }
            }
            "#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // negative integer value
            let json_input = r#"
            {
                "thread_2": {
                    "mismatch_action": "trap",
                    "match_action": "allow",
                    "filter": [
                        {
                            "syscall": "ioctl",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "qword",
                                    "op": "eq",
                                    "val": -1846
                                }
                            ]
                        }
                    ]
                }
            }
            "#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // float value
            let json_input = r#"
            {
                "thread_2": {
                    "mismatch_action": "trap",
                    "match_action": "allow",
                    "filter": [
                        {
                            "syscall": "ioctl",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "qword",
                                    "op": "eq",
                                    "val": 1846.4
                                }
                            ]
                        }
                    ]
                }
            }
            "#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // invalid comment
            let json_input = r#"
            {
                "thread_2": {
                    "mismatch_action": "trap",
                    "match_action": "allow",
                    "filter": [
                        {
                            "syscall": "ioctl",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "qword",
                                    "op": "eq",
                                    "val": 14,
                                    "comment": 15
                                }
                            ]
                        }
                    ]
                }
            }
            "#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());

            // duplicate filter keys
            let json_input = r#"
            {
                "thread_1": {
                    "mismatch_action": "trap",
                    "match_action": "allow",
                    "filter": []
                },
                "thread_1": {
                    "mismatch_action": "trap",
                    "match_action": "allow",
                    "filter": []
                }
            }
            "#;
            assert!(compiler.compile(json_input.as_bytes()).is_err());
        }

        // test with correctly formed JSON
        {
            // empty JSON file
            let json_input = "{}";

            assert_eq!(compiler.compile(json_input.as_bytes()).unwrap().len(), 0);

            // empty Filter
            let json_input =
                r#"{"a": {"match_action": "allow", "mismatch_action":"log", "filter": []}}"#;
            assert!(compiler.compile(json_input.as_bytes()).is_ok());

            // action fields using aliases
            let json_input = r#"{
                "a": {
                    "default_action":"log",
                    "filter_action": "allow",
                    "filter": []
                }
            }"#;
            let filter_with_aliases = compiler.compile(json_input.as_bytes()).unwrap();
            let json_input = r#"{
                    "a": {
                        "mismatch_action":"log",
                        "match_action": "allow",
                        "filter": []
                    }
                }"#;
            let filter_without_aliases = compiler.compile(json_input.as_bytes()).unwrap();
            assert_eq!(
                filter_with_aliases.get("a").unwrap(),
                filter_without_aliases.get("a").unwrap()
            );

            // action fields using combined action fields (with and without aliases)
            let json_input = r#"{
                "a": {
                    "default_action":"log",
                    "filter_action": "allow",
                    "filter": []
                }
            }"#;
            let filter_without_aliases = compiler.compile(json_input.as_bytes()).unwrap();
            assert_eq!(
                filter_with_aliases.get("a").unwrap(),
                filter_without_aliases.get("a").unwrap()
            );

            // correctly formed JSON filter
            let json_input = r#"
            {
                "thread_1": {
                    "mismatch_action": {
                        "errno": 12
                    },
                    "match_action": "allow",
                    "filter": [
                        {
                            "syscall": "openat"
                        },
                        {
                            "syscall": "close"
                        },
                        {
                            "syscall": "read"
                        },
                        {
                            "syscall": "futex",
                            "args": [
                                {
                                    "index": 2,
                                    "type": "dword",
                                    "op": "le",
                                    "val": 65
                                },
                                {
                                    "index": 1,
                                    "type": "qword",
                                    "op": "ne",
                                    "val": 80
                                }
                            ]
                        },
                        {
                            "syscall": "futex",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "qword",
                                    "op": "gt",
                                    "val": 65
                                },
                                {
                                    "index": 1,
                                    "type": "qword",
                                    "op": "lt",
                                    "val": 80
                                }
                            ]
                        },
                        {
                            "syscall": "futex",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "qword",
                                    "op": "ge",
                                    "val": 65,
                                    "comment": "dummy comment"
                                }
                            ]
                        },
                        {
                            "syscall": "ioctl",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "dword",
                                    "op": {
                                        "masked_eq": 100
                                    },
                                    "val": 65
                                }
                            ]
                        }
                    ]
                },
                "thread_2": {
                    "mismatch_action": "trap",
                    "match_action": "allow",
                    "filter": [
                        {
                            "syscall": "ioctl",
                            "comment": "dummy comment",
                            "args": [
                                {
                                    "index": 3,
                                    "type": "dword",
                                    "op": "eq",
                                    "val": 65,
                                    "comment": "dummy comment"
                                }
                            ]
                        }
                    ]
                }
            }
            "#;
            // safe because we know the string is UTF-8

            let mut filters = HashMap::new();
            filters.insert(
                "thread_1".to_string(),
                SeccompFilter::new(
                    vec![
                        (libc::SYS_openat, vec![]),
                        (libc::SYS_close, vec![]),
                        (libc::SYS_read, vec![]),
                        (
                            libc::SYS_futex,
                            vec![
                                SeccompRule::new(vec![
                                    Cond::new(2, Dword, Le, 65).unwrap(),
                                    Cond::new(1, Qword, Ne, 80).unwrap(),
                                ])
                                .unwrap(),
                                SeccompRule::new(vec![
                                    Cond::new(3, Qword, Gt, 65).unwrap(),
                                    Cond::new(1, Qword, Lt, 80).unwrap(),
                                ])
                                .unwrap(),
                                SeccompRule::new(vec![Cond::new(3, Qword, Ge, 65).unwrap()])
                                    .unwrap(),
                            ],
                        ),
                        (
                            libc::SYS_ioctl,
                            vec![SeccompRule::new(vec![
                                Cond::new(3, Dword, MaskedEq(100), 65).unwrap()
                            ])
                            .unwrap()],
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    SeccompAction::Errno(12),
                    SeccompAction::Allow,
                    ARCH.try_into().unwrap(),
                )
                .unwrap(),
            );

            filters.insert(
                "thread_2".to_string(),
                SeccompFilter::new(
                    vec![(
                        libc::SYS_ioctl,
                        vec![SeccompRule::new(vec![Cond::new(3, Dword, Eq, 65).unwrap()]).unwrap()],
                    )]
                    .into_iter()
                    .collect(),
                    SeccompAction::Trap,
                    SeccompAction::Allow,
                    ARCH.try_into().unwrap(),
                )
                .unwrap(),
            );

            // sort the HashMaps by key and transform into vectors, to make comparison possible
            let mut v1: Vec<_> = filters.into_iter().collect();
            v1.sort_by(|x, y| x.0.cmp(&y.0));

            let mut v2: Vec<_> = compiler
                .compile(json_input.as_bytes())
                .unwrap()
                .into_iter()
                .collect();
            v2.sort_by(|x, y| x.0.cmp(&y.0));
            assert_eq!(v1, v2);
        }
    }
}
