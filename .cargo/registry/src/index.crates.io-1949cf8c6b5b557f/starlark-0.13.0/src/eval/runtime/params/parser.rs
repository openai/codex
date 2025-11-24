/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::slice;

use starlark_syntax::internal_error;
use starlark_syntax::other_error;

use crate::values::UnpackValue;
use crate::values::Value;

/// Parse a series of parameters which were specified by
/// [`ParametersSpec`](crate::eval::ParametersSpec).
///
/// This is created with [`ParametersSpec::parser`](crate::eval::ParametersSpec::parser).
pub struct ParametersParser<'v, 'a> {
    // Invariant: `slots` and `names` are the same length.
    slots: slice::Iter<'a, Option<Value<'v>>>,
    names: slice::Iter<'a, String>,
}

impl<'v, 'a> ParametersParser<'v, 'a> {
    /// Create a parameter parser, which stored parameters into provided slots reference.
    pub(crate) fn new(slots: &'a [Option<Value<'v>>], names: &'a [String]) -> Self {
        // This assertion is important because we get unchecked in `get_next`.
        assert_eq!(slots.len(), names.len());
        ParametersParser {
            slots: slots.iter(),
            names: names.iter(),
        }
    }

    #[inline]
    fn get_next(&mut self) -> anyhow::Result<(Option<Value<'v>>, &'a str)> {
        let Some(v) = self.slots.next() else {
            return Err(
                internal_error!("Requesting more parameters than were specified").into_anyhow(),
            );
        };
        // SAFETY: struct fields invariant.
        let name = unsafe { self.names.next().unwrap_unchecked() };
        Ok((*v, name))
    }

    /// Obtain the next optional parameter (without a default value).
    pub fn next_opt<T: UnpackValue<'v>>(&mut self) -> anyhow::Result<Option<T>> {
        match self.get_next()? {
            (None, _) => Ok(None),
            (Some(v), name) => Ok(Some(T::unpack_named_param(v, name)?)),
        }
    }

    /// Obtain the next parameter. Fail if the parameter is optional and not provided.
    pub fn next<T: UnpackValue<'v>>(&mut self) -> anyhow::Result<T> {
        let (v, name) = self.get_next()?;
        let Some(v) = v else {
            return Err(other_error!(
                "Requested non-optional param {name} which was declared optional in signature"
            )
            .into_anyhow());
        };
        T::unpack_named_param(v, name)
    }

    #[inline]
    pub(crate) fn is_eof(&self) -> bool {
        self.slots.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::assert::Assert;
    use crate::docs::DocParam;
    use crate::docs::DocParams;
    use crate::docs::DocString;
    use crate::docs::DocStringKind;
    use crate::eval::compiler::def::FrozenDef;
    use crate::eval::runtime::params::display::PARAM_FMT_OPTIONAL;
    use crate::eval::runtime::params::spec::ParametersSpec;
    use crate::eval::ParametersSpecParam;
    use crate::typing::Ty;
    use crate::values::FrozenValue;

    #[test]
    fn test_documentation() -> anyhow::Result<()> {
        // Make sure that documentation for some odder parameter specs works properly.
        let p = ParametersSpec::<FrozenValue>::new_parts(
            "f",
            [],
            [],
            true,
            [
                ("a", ParametersSpecParam::Optional),
                ("b", ParametersSpecParam::Optional),
            ],
            false,
        );

        let expected = DocParams {
            args: Some(DocParam {
                name: "args".to_owned(),
                docs: None,
                typ: Ty::any(),
                default_value: None,
            }),
            named_only: vec![
                DocParam {
                    name: "a".to_owned(),
                    docs: None,
                    typ: Ty::int(),
                    default_value: Some(PARAM_FMT_OPTIONAL.to_owned()),
                },
                DocParam {
                    name: "b".to_owned(),
                    docs: DocString::from_docstring(DocStringKind::Rust, "param b docs"),
                    typ: Ty::any(),
                    default_value: Some(PARAM_FMT_OPTIONAL.to_owned()),
                },
            ],
            pos_only: Vec::new(),
            pos_or_named: Vec::new(),
            kwargs: None,
        };
        let types = vec![Ty::any(), Ty::int(), Ty::any()];
        let mut docs = HashMap::new();
        docs.insert("a".to_owned(), None);
        docs.insert(
            "b".to_owned(),
            DocString::from_docstring(DocStringKind::Rust, "param b docs"),
        );

        let params = p.documentation(types, docs);
        assert_eq!(expected, params);
        Ok(())
    }

    #[test]
    fn test_parameters_str() {
        fn test(sig: &str) {
            let a = Assert::new();
            let f = a
                .pass_module(&format!("def f({sig}): pass"))
                .get("f")
                .unwrap();
            assert_eq!(sig, &f.value().parameters_spec().unwrap().parameters_str());
        }

        test("");

        test("a, b, c, d, e, f, g, h, *args, **kwargs");

        test("*, a");
        test("x, *, a");

        test("*args, a");
        test("x, *args, a");

        test("**kwargs");
        test("a, **kwargs");
    }

    #[test]
    fn test_can_fill_with_args() {
        fn test(sig: &str, pos: usize, names: &[&str], expected: bool) {
            let a = Assert::new();
            let module = a.pass_module(&format!("def f({}): pass", sig));
            let f = module.get("f").unwrap().downcast::<FrozenDef>().unwrap();
            let parameters_spec = &f.parameters;
            assert_eq!(expected, parameters_spec.can_fill_with_args(pos, names));
        }

        test("", 0, &[], true);
        test("", 1, &[], false);
        test("", 0, &["a"], false);

        test("a", 1, &[], true);
        test("a", 0, &["a"], true);
        test("a", 1, &["a"], false);
        test("a", 0, &["x"], false);

        test("a, b = 1", 1, &[], true);
        test("a, b = 1", 2, &[], true);
        test("a, b = 1", 0, &["a"], true);
        test("a, b = 1", 0, &["b"], false);
        test("a, b = 1", 0, &["a", "b"], true);

        test("*, a", 0, &[], false);
        test("*, a", 1, &[], false);
        test("*, a", 0, &["a"], true);

        test("a, *args", 0, &[], false);
        test("a, *args", 1, &[], true);
        test("a, *args", 10, &[], true);

        test("*args, b", 0, &[], false);
        test("*args, b", 1, &[], false);
        test("*args, b", 0, &["b"], true);

        test("**kwargs", 0, &[], true);
        test("**kwargs", 0, &["a"], true);
        test("**kwargs", 1, &[], false);

        // No test for positional-only args because we can't create them in starlark.
    }
}
