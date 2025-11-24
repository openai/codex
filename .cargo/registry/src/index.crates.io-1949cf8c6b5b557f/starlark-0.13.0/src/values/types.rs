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

pub mod any;
pub mod any_array;
pub mod any_complex;
pub mod array;
pub mod bigint;
pub mod bool;
pub mod dict;
pub(crate) mod ellipsis;
pub mod enumeration;
pub mod exported_name;
pub mod float;
pub mod function;
pub mod int;
pub(crate) mod known_methods;
pub mod list;
pub mod list_or_tuple;
pub mod namespace;
pub mod none;
pub(crate) mod num;
pub mod range;
pub mod record;
pub mod set;
pub mod starlark_value_as_type;
pub mod string;
pub mod structs;
pub mod tuple;
pub(crate) mod type_instance_id;
pub(crate) mod unbound;
