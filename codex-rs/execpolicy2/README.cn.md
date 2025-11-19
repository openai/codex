# codex-execpolicy2（中文说明）

## 概览

- 基于 `prefix_rule(pattern=[...], decision?, match?, not_match?)` 的策略引擎和 CLI。
- 当前版本只覆盖了计划中的 execpolicy v2 语言中的“前缀规则”子集，后续会扩展为更丰富的语言。
- 命令行参数按顺序匹配；`pattern` 中的任意元素都可以是列表，用来表示多个备选 token。`decision` 默认值为 `allow`，可取的值包括：`allow`、`prompt`、`forbidden`。
- `match` / `not_match` 用来提供示例调用，这些示例会在策略加载时验证（可以理解为内联的单元测试）；示例既可以是 token 数组，也可以是字符串（字符串会用 `shlex` 进行分词）。
- CLI 始终打印评估结果的 JSON 序列化（无论是否有匹配）。

## 策略形状（Policy shapes）

前缀规则使用 Starlark 语法：

```starlark
prefix_rule(
    pattern = ["cmd", ["alt1", "alt2"]], # 有序 token；列表元素表示备选
    decision = "prompt",                 # allow | prompt | forbidden；默认 allow
    match = [["cmd", "alt1"], "cmd alt2"],           # 必须匹配该规则的示例
    not_match = [["cmd", "oops"], "cmd alt3"],       # 必须不能匹配该规则的示例
)
```

## 响应形状（Response shapes）

- 有匹配时：

```json
{
  "match": {
    "decision": "allow|prompt|forbidden",
    "matchedRules": [
      {
        "prefixRuleMatch": {
          "matchedPrefix": ["<token>", "..."],
          "decision": "allow|prompt|forbidden"
        }
      }
    ]
  }
}
```

- 无匹配时：

```json
"noMatch"
```

- `matchedRules` 列出所有前缀命中的规则；`matchedPrefix` 是实际命中的前缀。
- 最终生效的 `decision` 是所有匹配中“最严格”的一个（`forbidden` > `prompt` > `allow`）。

## CLI 使用

- 使用一个或多个策略文件（例如 `src/default.codexpolicy`）检查命令：

```bash
cargo run -p codex-execpolicy2 -- check --policy path/to/policy.codexpolicy git status
```

- 通过传入多个 `--policy` 参数合并规则，按提供顺序依次评估：

```bash
cargo run -p codex-execpolicy2 -- check --policy base.codexpolicy --policy overrides.codexpolicy git status
```

- 默认输出为按行分隔的 JSON；如需漂亮打印，可添加 `--pretty`。
- 示例结果：
  - 有匹配：`{"match": { ... "decision": "allow" ... }}`
  - 无匹配：`"noMatch"`

