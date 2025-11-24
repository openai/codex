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

//! Bytecode interpreter.

pub(crate) mod addr;
pub(crate) mod bytecode;
pub(crate) mod call;
pub(crate) mod compiler;
pub(crate) mod definitely_assigned;
pub(crate) mod for_loop;
pub(crate) mod frame;
pub(crate) mod if_debug;
pub(crate) mod instr;
pub(crate) mod instr_arg;
pub(crate) mod instr_impl;
pub(crate) mod instrs;
pub(crate) mod native_function;
pub(crate) mod opcode;
pub(crate) mod repr;
pub(crate) mod slow_arg;
pub(crate) mod stack_ptr;
pub(crate) mod writer;
