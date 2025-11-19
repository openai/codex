# codex_file_search（中文说明）

Codex 使用的快速模糊文件搜索工具。

内部使用 <https://crates.io/crates/ignore>（`ripgrep` 也在用它）遍历目录，遵守 `.gitignore` 等规则构建文件列表，然后用 <https://crates.io/crates/nucleo-matcher> 在这些文件路径上对用户提供的 `PATTERN` 做模糊匹配。

