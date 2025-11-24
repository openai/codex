/*
 * Copyright 2018 The Starlark in Rust Authors.
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

use allocative::Allocative;
use derive_more::Display;
use starlark_derive::starlark_attrs;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::assert::Assert;
use crate::starlark_simple_value;
use crate::values::StarlarkAttrs;
use crate::values::StarlarkValue;

#[test]
fn test_derive_attrs() {
    #[derive(
        Debug,
        StarlarkAttrs,
        Display,
        ProvidesStaticType,
        NoSerialize,
        Allocative
    )]
    #[display("{:?}", self)]
    struct Example {
        hello: String,
        #[starlark(skip)]
        answer: i64,
        #[starlark(clone)]
        nested: Nested,
        r#type: i64,
        r#escaped: String,
    }
    starlark_simple_value!(Example);

    #[starlark_value(type = "example")]
    impl<'v> StarlarkValue<'v> for Example {
        starlark_attrs!();
    }

    #[derive(
        Debug,
        Clone,
        StarlarkAttrs,
        Display,
        ProvidesStaticType,
        NoSerialize,
        Allocative
    )]
    #[display("{}", foo)]
    struct Nested {
        foo: String,
    }
    starlark_simple_value!(Nested);

    #[starlark_value(type = "nested")]
    impl<'v> StarlarkValue<'v> for Nested {
        starlark_attrs!();
    }

    let mut a = Assert::new();
    a.globals_add(|gb| {
        gb.set(
            "example",
            Example {
                hello: "world".to_owned(),
                answer: 42,
                nested: Nested {
                    foo: "bar".to_owned(),
                },
                r#type: 1,
                r#escaped: "baz".to_owned(),
            },
        )
    });

    // dir
    a.eq(
        "dir(example)",
        "[\"escaped\", \"hello\", \"nested\", \"type\"]",
    );

    // getattr
    a.eq("example.hello", "\"world\"");
    a.eq("example.nested.foo", "\"bar\"");
    a.eq("example.type", "1");
    a.eq("example.escaped", "\"baz\"");

    // hasattr
    a.is_true("not hasattr(example, \"answer\")");
    a.is_true("hasattr(example, \"hello\")");
    a.is_true("hasattr(example, \"nested\")");
    a.is_true("hasattr(example, \"type\")");
    a.is_true("not hasattr(example, \"r#type\")");
    a.is_true("hasattr(example, \"escaped\")");
    a.is_true("not hasattr(example, \"r#escaped\")");
}
