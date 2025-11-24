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

fn lalrpop() {
    let source = "src/syntax/grammar.lalrpop";
    println!("cargo:rerun-if-changed={}", source);
    lalrpop::Configuration::new()
        .use_cargo_dir_conventions()
        .emit_report(true)
        .process_file(source)
        .unwrap();
}

fn main() {
    lalrpop();
}
