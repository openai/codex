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

use allocative::Allocative;
use dupe::Dupe;

use crate::values::record::Record;
use crate::values::types::type_instance_id::TypeInstanceId;
use crate::values::typing::type_compiled::matcher::TypeMatcher;
use crate::values::Value;

#[derive(Hash, Debug, Eq, PartialEq, Clone, Dupe, Allocative)]
pub(crate) struct RecordTypeMatcher {
    pub(crate) id: TypeInstanceId,
}

impl TypeMatcher for RecordTypeMatcher {
    fn matches(&self, value: Value) -> bool {
        match Record::from_value(value) {
            None => false,
            Some(record) => record.record_type_id() == self.id,
        }
    }
}
