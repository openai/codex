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

//! Utility to write files in formats understood by `flamegraph.pl`.

use std::fmt::Write;

use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use crate::util::arc_str::ArcStr;

/// Node in flamegraph tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FlameGraphNode {
    children: SmallMap<ArcStr, FlameGraphNode>,
    value: Option<u64>,
}

/// Profiling data as flame tree.
///
/// Can be written to `flamegraph.pl` format.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FlameGraphData {
    root: FlameGraphNode,
}

impl FlameGraphNode {
    fn write<'a>(&'a self, writer: &mut FlameGraphWriter, stack: &mut Vec<&'a str>) {
        if let Some(value) = self.value {
            writer.write(stack.iter().copied(), value);
        }
        for (k, v) in self.children.iter() {
            stack.push(k);
            v.write(writer, stack);
            stack.pop().unwrap();
        }
    }

    /// Add value to the node.
    pub(crate) fn add(&mut self, value: u64) {
        match &mut self.value {
            None => self.value = Some(value),
            Some(v) => *v += value,
        }
    }

    pub(crate) fn merge(&mut self, other: &FlameGraphNode) {
        if let Some(value) = other.value {
            self.add(value);
        }

        for (k, v) in &other.children {
            self.child(k.dupe()).merge(v);
        }
    }

    /// Get or create a child node.
    pub(crate) fn child(&mut self, name: ArcStr) -> &mut FlameGraphNode {
        self.children.entry(name).or_default()
    }
}

impl FlameGraphData {
    pub(crate) fn write(&self) -> String {
        let mut writer = FlameGraphWriter::new();
        let mut stack = Vec::new();
        self.root.write(&mut writer, &mut stack);
        assert!(stack.is_empty());
        writer.finish()
    }

    pub(crate) fn root(&mut self) -> &mut FlameGraphNode {
        &mut self.root
    }

    pub(crate) fn merge<'a>(
        graphs: impl IntoIterator<Item = &'a FlameGraphData>,
    ) -> FlameGraphData {
        let mut result = FlameGraphData::default();
        for graph in graphs {
            result.root.merge(&graph.root);
        }
        result
    }
}

pub(crate) struct FlameGraphWriter {
    buf: String,
}

impl FlameGraphWriter {
    pub(crate) fn new() -> FlameGraphWriter {
        FlameGraphWriter { buf: String::new() }
    }

    pub(crate) fn write<'s>(&mut self, key: impl IntoIterator<Item = &'s str>, value: u64) {
        let key = key.into_iter().collect::<Vec<_>>();
        if key.is_empty() {
            writeln!(self.buf, "(unknown) {}", value).unwrap();
        } else {
            writeln!(self.buf, "{} {}", key.join(";"), value).unwrap();
        }
    }

    pub(crate) fn finish(self) -> String {
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use crate::eval::runtime::profile::flamegraph::FlameGraphData;
    use crate::eval::runtime::profile::flamegraph::FlameGraphWriter;

    #[test]
    fn test_flamegraph_writer() {
        let mut writer = FlameGraphWriter::new();
        writer.write(["aa", "bb"], 20);
        assert_eq!("aa;bb 20\n", writer.finish());
    }

    #[test]
    fn test_flamegraph_data() {
        let mut data = FlameGraphData::default();
        data.root().child("a".into()).add(10);
        data.root().child("a".into()).child("b".into()).add(20);
        data.root().child("a".into()).add(30);
        let data = data.write();
        assert_eq!("a 40\na;b 20\n", data);
    }

    #[test]
    fn test_merge() {
        let mut a = FlameGraphData::default();
        a.root().add(10);
        a.root().child("a".into()).add(100);
        a.root().child("b".into()).child("c".into()).add(1000);
        let mut b = FlameGraphData::default();
        b.root().add(20);
        b.root().child("a".into()).add(200);

        let c = FlameGraphData::merge([&a, &b]);

        let mut expected = FlameGraphData::default();
        expected.root().add(30);
        expected.root().child("a".into()).add(300);
        expected
            .root()
            .child("b".into())
            .child("c".into())
            .add(1000);

        assert_eq!(expected, c);
    }
}
