load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_binary")
load("//:defs.bzl", "workspace_root_test")

_WORKSPACE_ROOT_MARKER = "//codex-rs/utils/cargo-bin:repo_root.marker"

def codex_e2e_benchmark(name, binaries = [], data = [], deps = []):
    """Defines a Bazel-only Divan end-to-end benchmark.

    The benchmark source lives at `e2e_benches/<name>.rs`, with hyphens in
    `name` replaced by underscores. `binaries` are runtime executables made
    available through the same `CARGO_BIN_EXE_*` bridge used by Rust tests.

    Args:
        name: Stem for the generated `<name>-bench` target and benchmark.
        binaries: Runtime executable labels that the benchmark spawns.
        data: Additional runtime files needed by the benchmark.
        deps: Additional Rust dependencies beyond the crate's Cargo deps.
    """
    benchmark_name = name.replace("-", "_")
    source = "e2e_benches/{}.rs".format(benchmark_name)
    binary_name = name + "-bench-bin"
    runfile_env = {
        binary: "CARGO_BIN_EXE_" + native.package_relative_label(binary).name
        for binary in binaries
    }

    rust_binary(
        name = binary_name,
        testonly = True,
        srcs = [source],
        crate_name = benchmark_name + "_bench",
        crate_root = source,
        deps = all_crate_deps(
            normal = True,
            normal_dev = True,
        ) + [
            "@crates//:divan",
        ] + deps,
    )

    workspace_root_test(
        name = name + "-bench",
        args = [
            "--bench",
            benchmark_name,
        ],
        data = data,
        # Keep path resolution inside the wrapper so manifest-only runfiles
        # work on every supported host platform.
        runfile_env = runfile_env,
        tags = ["manual"],
        test_bin = ":" + binary_name,
        visibility = ["//codex-rs:__pkg__"],
        workspace_root_marker = _WORKSPACE_ROOT_MARKER,
    )
