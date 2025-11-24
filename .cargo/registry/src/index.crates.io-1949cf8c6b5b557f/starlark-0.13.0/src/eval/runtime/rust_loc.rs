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

/// Initialize `$loc` to `FrozenRef<FrameSpan>` with rust file and line number.
macro_rules! rust_loc {
    () => {{
        use crate::codemap::CodeMap;
        use crate::codemap::NativeCodeMap;
        use crate::eval::runtime::frame_span::FrameSpan;
        use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
        use crate::values::FrozenRef;

        static NATIVE_CODEMAP: NativeCodeMap = NativeCodeMap::new(file!(), line!(), column!());
        static CODEMAP: CodeMap = NATIVE_CODEMAP.to_codemap();
        static FROZEN_FILE_SPAN: FrameSpan = FrameSpan::new(FrozenFileSpan::new_unchecked(
            FrozenRef::new(&CODEMAP),
            NativeCodeMap::FULL_SPAN,
        ));
        static LOC: FrozenRef<'static, FrameSpan> = FrozenRef::new(&FROZEN_FILE_SPAN);
        LOC
    }};
}

pub(crate) use rust_loc;

#[cfg(test)]
mod tests {
    use starlark_derive::starlark_module;
    use starlark_syntax::error::StarlarkResultExt;

    use crate as starlark;
    use crate::assert::Assert;
    use crate::environment::GlobalsBuilder;
    use crate::eval::Arguments;
    use crate::eval::Evaluator;
    use crate::values::Value;

    #[starlark_module]
    fn rust_loc_globals(globals: &mut GlobalsBuilder) {
        fn invoke<'v>(f: Value<'v>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
            f.invoke_with_loc(Some(rust_loc!()), &Arguments::default(), eval)
                .into_anyhow_result()
        }
    }

    #[test]
    fn test_rust_loc() {
        let mut a = Assert::new();
        a.globals_add(rust_loc_globals);
        let err = a.fail("invoke(fail)", "");
        let err = err.to_string();
        // Stack trace should contain invocation in `invoke`.
        assert!(
            // Make test compatible with Windows.
            err.replace('\\', "/")
                .contains("src/eval/runtime/rust_loc.rs"),
            "output: {:?}",
            err
        );
        assert!(err.contains("<native>"), "output: {:?}", err);
    }
}
