/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(test)]

use std::env;
use std::fmt::Write;
use std::fs;

use crate::Allocative;
use crate::FlameGraphBuilder;

const REGENERATE_VAR_NAME: &str = "ALLOCATIVE_REGENERATE_TESTS";

fn golden_header() -> String {
    let at = "@";
    format!(
        "{at}generated\n\
    To regenerate, run:\n\
    ```\n\
    {REGENERATE_VAR_NAME}=1 cargo test -p allocative\n\
    ```\n"
    )
}

fn make_golden<T: Allocative>(value: &T) -> (String, String) {
    let mut builder = FlameGraphBuilder::default();
    builder.visit_root(value);
    let flamegraph = builder.finish_and_write_flame_graph();
    // Because crate name is `allocative_unittest` in fbcode buck2 tests.
    let flamegraph = flamegraph.replace("allocative_unittest::", "allocative::");
    let mut flamegraph_svg: Vec<u8> = Vec::new();
    let mut inferno_options = inferno::flamegraph::Options::default();
    inferno_options.deterministic = true;
    inferno_options.pretty_xml = true;
    inferno::flamegraph::from_reader(
        &mut inferno_options,
        flamegraph.as_bytes(),
        &mut flamegraph_svg,
    )
    .unwrap();
    let flamegraph_svg = String::from_utf8(flamegraph_svg).unwrap();

    let flamegraph = format!(
        "{header}{flamegraph}",
        header = golden_header()
            .lines()
            .fold(String::new(), |mut output, line| {
                let _ = writeln!(output, "# {}", line);
                output
            })
    );

    let flamegraph_svg = flamegraph_svg.replace(
        "<!DOCTYPE",
        &format!("\n<!--\n{}-->\n<!DOCTYPE", golden_header()),
    );

    (flamegraph, flamegraph_svg)
}

fn type_name_to_path(type_name: &str) -> String {
    let path = type_name.strip_suffix("::Marker").unwrap();
    let path = if let Some(path) = path.strip_prefix("allocative::") {
        path
    } else {
        // In buck2 test crate has suffix `_unittest`.
        path.strip_prefix("allocative_unittest::").unwrap()
    };
    path.replace("::tests::", "_").replace("::", "/")
}

pub(crate) fn golden_test_impl<T: Allocative>(value: &T, type_name: &str) {
    let path = type_name_to_path(type_name);
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` variable must be set");

    let flamegraph_src_path = format!("{manifest_dir}/src/{path}.src");
    let flamegraph_svg_path = format!("{manifest_dir}/src/{path}.svg");

    let (flamegraph, flamegraph_svg) = make_golden(value);
    if env::var(REGENERATE_VAR_NAME).is_ok() {
        fs::write(flamegraph_src_path, &flamegraph).unwrap();
        fs::write(flamegraph_svg_path, flamegraph_svg).unwrap();
    } else {
        let expected_flamegraph = fs::read_to_string(flamegraph_src_path)
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(expected_flamegraph, flamegraph);
    }
}

macro_rules! golden_test {
    ($expr:expr) => {{
        struct Marker;
        crate::golden::golden_test_impl($expr, std::any::type_name::<Marker>());
    }};
}

pub(crate) use golden_test;
