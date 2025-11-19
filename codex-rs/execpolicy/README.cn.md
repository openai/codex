# codex_execpolicy（中文说明）

这个库的目标是对一个待执行的 [`execv(3)`](https://linux.die.net/man/3/execv) 命令进行分类，结果之一：

- `safe`：命令被认为是“安全的”（\*）；
- `match`：命令命中了策略中的某条规则，但是否安全还需要调用方结合即将写入的文件路径自行判断；
- `forbidden`：命令不允许被执行；
- `unverified`：无法判断安全性，需要由用户决策。

（\*）某个 `execv(3)` 调用是否“安全”通常取决于比参数本身更多的上下文信息。例如，如果你信任一个自动化 agent 可以在源码树下写文件，那么 `/bin/cp foo bar` 是否安全，取决于调用进程的 `getcwd(3)` 以及将 `foo`、`bar` 结合 `getcwd()` 后 `realpath` 出来的实际路径。

因此，验证器不会简单返回布尔值，而是返回一个结构化结果，让客户端基于该结果来判断这个 `execv()` 调用是否安全。

示例：检查命令 `ls -l foo`：

```shell
cargo run -- check ls -l foo | jq
```

命令会以退出码 `0` 结束，并在 stdout 打印类似：

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

关键点：

- `foo` 被标记为 `ReadableFile`，调用方需要依据 `getcwd()` 解析并 `realpath` 这个路径（因为可能是符号链接），再根据策略判断是否允许读取。
- 虽然参数中的可执行文件名是 `ls`，但 `"system_path"` 给出了 `/bin/ls` 和 `/usr/bin/ls` 作为更可靠的候选路径，以避免使用用户 `$PATH` 中其他可疑的 `ls`。如果这些路径存在，推荐把它们作为 `execv(3)` 的第一个参数，而不是裸用 `ls`。

需要注意的是，“安全”并不意味着命令必然执行成功。比如 `cat /Users/mbolin/code/codex/README.md`，如果系统认为 `/Users/mbolin/code/codex` 下的文件都可以被 agent 读取，那么该命令在策略上是“安全”的；但如果 `README.md` 实际并不存在，运行时仍会失败（但至少不会读取未授权的文件）。

## 策略（Policy）

当前默认策略定义在 crate 内的 [`default.policy`](./src/default.policy) 中。

系统使用 [Starlark](https://bazel.build/rules/language) 作为策略文件格式，因为相比 JSON/YAML，它支持“宏”等结构，又不会牺牲安全性和可重复性（具体实现依赖 [`starlark-rust`](https://github.com/facebook/starlark-rust)）。

示例规则：

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

含义：

- `cp` 可以搭配列出的这些“flag”（不带参数的选项）使用；
- 起始的 `ARG_RFILES` 表示期望一个或多个“可读文件”参数；
- 最后的 `ARG_WFILE` 表示期望一个“可写文件”参数；
- `should_match`/`should_not_match` 提供了一组内联示例，在加载 `.policy` 文件时会自动验证，相当于轻量的单元测试。

策略语言仍在演进中，我们需要不断扩展它，以在尽量不放宽安全边界的前提下接受更多“安全命令”。`default.policy` 的完整性通过[单元测试](./tests) 进行校验；CLI 还支持 `--policy` 选项来加载自定义策略进行临时测试。

## 输出类型：`match`

继续以上 `cp` 的例子，由于规则中包含 `ARG_WFILE`，因此这类命令会被标记为 `match` 而不是 `safe`：

```shell
cargo run -- check cp src1 src2 dest | jq
```

如果调用方想考虑放行这个命令，应该解析 JSON，找到所有 `WriteableFile` 参数并判断它们是否允许写入：

```json
{
  "result": "match",
  "match": {
    "program": "cp",
    "flags": [],
    "opts": [],
    "args": [
      { "index": 0, "type": "ReadableFile", "value": "src1" },
      { "index": 1, "type": "ReadableFile", "value": "src2" },
      { "index": 2, "type": "WriteableFile", "value": "dest" }
    ],
    "system_path": ["/bin/cp", "/usr/bin/cp"]
  }
}
```

在 `match` 情况下，退出码仍为 `0`，除非显式指定 `--require-safe`，此时遇到 `match` 会使用退出码 `12`。

## 输出类型：`forbidden`

也可以定义某些命令一旦匹配就直接标记为 _forbidden_。例如我们永远不希望 agent 执行 `applied deploy`，则可以：

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

要让规则标记为 forbidden，必须提供 `forbidden` 字段作为原因，这个原因会出现在输出中：

```shell
cargo run -- check applied deploy | jq
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
            "type": { "Literal": "deploy" },
            "value": "deploy"
          }
        ],
        "system_path": []
      }
    }
  }
}
```

