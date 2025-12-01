# codex-execpolicy-legacy

This crate hosts the original execpolicy implementation. The newer prefix-rule
engine lives in `codex-execpolicy`.

The goal of this library is to classify a proposed [`execv(3)`](https://linux.die.net/man/3/execv) command into one of the following states:

- `safe` The command is safe to run (\*).
- `match` The command matched a rule in the policy, but the caller should decide whether it is safe to run based on the files it will write.
- `forbidden` The command is not allowed to be run.
- `unverified` The safety cannot be determined: make the user decide.

(\*) Whether an `execv(3)` call should be considered "safe" often requires additional context beyond the arguments to `execv()` itself. For example, if you trust an autonomous software agent to write files in your source tree, then deciding whether `/bin/cp foo bar` is "safe" depends on `getcwd(3)` for the calling process as well as the `realpath` of `foo` and `bar` when resolved against `getcwd()`.
To that end, rather than returning a boolean, the validator returns a structured result that the client is expected to use to determine the "safety" of the proposed `execv()` call.

For example, to check the command `ls -l foo`, the checker would be invoked as follows:

```shell
cargo run -p codex-execpolicy-legacy -- check ls -l foo | jq
```

It will exit with `0` and print the following to stdout:

```json
{
  "result": "safe",
  "match": {
    "program": "ls",
    "flags": [
      {
        "name": "-l"
      }
    ],
    "opts": [],
    "args": [
      {
        "index": 1,
        "type": "ReadableFile",
        "value": "foo"
      }
    ],
    "system_path": ["/bin/ls", "/usr/bin/ls"]
  }
}
```

Of note:

- `foo` is tagged as a `ReadableFile`, so the caller should resolve `foo` relative to `getcwd()` and `realpath` it (as it may be a symlink) to determine whether `foo` is safe to read.
- While the specified executable is `ls`, `"system_path"` offers `/bin/ls` and `/usr/bin/ls` as viable alternatives to avoid using whatever `ls` happens to appear first on the user's `$PATH`. If either exists on the host, it is recommended to use it as the first argument to `execv(3)` instead of `ls`.

Further, "safety" in this system is not a guarantee that the command will execute successfully. As an example, `cat /Users/mbolin/code/codex/README.md` may be considered "safe" if the system has decided the agent is allowed to read anything under `/Users/mbolin/code/codex`, but it will fail at runtime if `README.md` does not exist. (Though this is "safe" in that the agent did not read any files that it was not authorized to read.)

## Policy

Currently, the default policy is defined in [`default.policy`](./src/default.policy) within the crate.

The system uses [Starlark](https://bazel.build/rules/language) as the file format because, unlike something like JSON or YAML, it supports "macros" without compromising on safety or reproducibility. (Under the hood, we use [`starlark-rust`](https://github.com/facebook/starlark-rust) as the specific Starlark implementation.)

This policy contains "rules" such as:

```python
define_program(
    program="cp",
    options=[
        flag("-r"),
        flag("-R"),
        flag("--recursive"),
    ],
    args=[ARG_RFILES, ARG_WFILE],
    system_path=["/bin/cp", "/usr/bin/cp"],
    should_match=[
        ["foo", "bar"],
    ],
    should_not_match=[
        ["foo"],
    ],
)
```

This rule means that:

- `cp` can be used with any of the following flags (where "flag" means "an option that does not take an argument"): `-r`, `-R`, `--recursive`.
- The initial `ARG_RFILES` passed to `args` means that it expects one or more arguments that correspond to "readable files"
- The final `ARG_WFILE` passed to `args` means that it expects exactly one argument that corresponds to a "writeable file."
- As a means of a lightweight way of including a unit test alongside the definition, the `should_match` list is a list of examples of `execv(3)` args that should match the rule and `should_not_match` is a list of examples that should not match. These examples are verified when the `.policy` file is loaded.

Note that the language of the `.policy` file is still evolving, as we have to continue to expand it so it is sufficiently expressive to accept all commands we want to consider "safe" without allowing unsafe commands to pass through.

The integrity of `default.policy` is verified [via unit tests](./tests).

Further, the CLI supports a `--policy` option to specify a custom `.policy` file for ad-hoc testing.

## Output Type: `match`

Going back to the `cp` example, because the rule matches an `ARG_WFILE`, it will return `match` instead of `safe`:

```shell
cargo run -p codex-execpolicy-legacy -- check cp src1 src2 dest | jq
```

If the caller wants to consider allowing this command, it should parse the JSON to pick out the `WriteableFile` arguments and decide whether they are safe to write:

```json
{
  "result": "match",
  "match": {
    "program": "cp",
    "flags": [],
    "opts": [],
    "args": [
      {
        "index": 0,
        "type": "ReadableFile",
        "value": "src1"
      },
      {
        "index": 1,
        "type": "ReadableFile",
        "value": "src2"
      },
      {
        "index": 2,
        "type": "WriteableFile",
        "value": "dest"
      }
    ],
    "system_path": ["/bin/cp", "/usr/bin/cp"]
  }
}
```

Note the exit code is still `0` for a `match` unless the `--require-safe` flag is specified, in which case the exit code is `12`.

## Output Type: `forbidden`

It is also possible to define a rule that, if it matches a command, should flag it as _forbidden_. For example, we do not want agents to be able to run `applied deploy` _ever_, so we define the following rule:

```python
define_program(
    program="applied",
    args=["deploy"],
    forbidden="Infrastructure Risk: command contains 'applied deploy'",
    should_match=[
        ["deploy"],
    ],
    should_not_match=[
        ["lint"],
    ],
)
```

Note that for a rule to be forbidden, the `forbidden` keyword arg must be specified as the reason the command is forbidden. This will be included in the output:

```shell
cargo run -p codex-execpolicy-legacy -- check applied deploy | jq
```

```json
{
  "result": "forbidden",
  "reason": "Infrastructure Risk: command contains 'applied deploy'",
  "cause": {
    "Exec": {
      "exec": {
        "program": "applied",
        "flags": [],
        "opts": [],
        "args": [
          {
            "index": 0,
            "type": {
              "Literal": "deploy"
            },
            "value": "deploy"
          }
        ],
        "system_path": []
      }
    }
  }
}
```
