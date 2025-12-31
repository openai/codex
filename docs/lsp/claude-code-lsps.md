# Claude Code LSPs - 设计与实现分析

> 项目地址: https://github.com/Piebald-AI/claude-code-lsps

## 1. 项目概述

Claude Code LSPs 是一个 **Claude Code 插件市场(marketplace)**，提供 14 个 LSP (Language Server Protocol) 插件，用于为 Claude Code 集成多种语言服务器，实现代码智能功能。

### 1.1 核心价值

- **代码导航**: 跳转到定义 (goToDefinition)
- **悬停信息**: 显示符号文档 (hover)
- **符号列表**: 列出文件中所有符号 (documentSymbol)
- **引用查找**: 查找符号的所有引用 (findReferences)
- **工作区搜索**: 跨工作区搜索符号 (workspaceSymbol)

### 1.2 支持的语言

| 语言 | 插件名 | LSP 服务器 |
|------|--------|-----------|
| TypeScript/JavaScript | vtsls | vtsls |
| Rust | rust-analyzer | rust-analyzer |
| Python | pyright | pyright-langserver |
| Go | gopls | gopls |
| Java | jdtls | Eclipse JDT |
| Kotlin | kotlin-lsp | kotlin-lsp |
| C/C++ | clangd | clangd |
| PHP | phpactor | phpactor |
| Ruby | ruby-lsp | ruby-lsp |
| C# | omnisharp | omnisharp |
| PowerShell | powershell-editor-services | pwsh + PowerShellEditorServices |
| HTML/CSS | vscode-langservers | vscode-html/css-language-server |
| LaTeX | texlab | texlab |
| BSL/1C | bsl-lsp | bsl-language-server |

---

## 2. 项目架构

### 2.1 目录结构

```
claude-code-lsps/
├── .claude-plugin/
│   └── marketplace.json      # 市场清单 - 注册所有插件
├── CLAUDE.md                 # 开发者指南
├── README.md                 # 用户安装说明
│
├── vtsls/                    # TypeScript/JavaScript 插件
│   ├── plugin.json          # 插件元数据
│   └── .lsp.json            # LSP 配置
├── rust-analyzer/            # Rust 插件
│   ├── plugin.json
│   └── .lsp.json
├── pyright/                  # Python 插件
├── gopls/                    # Go 插件
├── jdtls/                    # Java 插件
├── kotlin-lsp/               # Kotlin 插件
├── clangd/                   # C/C++ 插件
├── phpactor/                 # PHP 插件
├── ruby-lsp/                 # Ruby 插件
├── omnisharp/                # C# 插件
├── powershell-editor-services/ # PowerShell 插件
├── vscode-langservers/       # HTML/CSS 插件 (多服务器)
├── texlab/                   # LaTeX 插件
└── bsl-lsp/                  # BSL/1C 插件
```

### 2.2 设计模式

采用 **声明式配置** 模式:
- 无需编写代码，所有配置通过 JSON 文件定义
- 每个插件由 2 个文件组成: `plugin.json` + `.lsp.json`
- 市场清单 `marketplace.json` 集中管理所有插件

---

## 3. 配置文件详解

### 3.1 marketplace.json - 市场清单

**位置**: `.claude-plugin/marketplace.json`

**作用**: 注册市场中的所有插件，Claude Code 通过此文件发现可用插件。

**结构**:
```json
{
  "name": "claude-code-lsps",
  "owner": {
    "name": "Piebald LLC",
    "email": "support@piebald.ai"
  },
  "plugins": [
    {
      "name": "rust-analyzer",
      "version": "0.1.0",
      "source": "./rust-analyzer",
      "description": "Rust language server integration with rust-analyzer",
      "category": "development",
      "tags": ["rust", "lsp"],
      "author": {
        "name": "Piebald LLC",
        "email": "support@piebald.ai"
      }
    }
    // ... 更多插件
  ]
}
```

**字段说明**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 市场名称 |
| `owner` | object | 市场所有者信息 |
| `plugins` | array | 插件列表 |
| `plugins[].name` | string | 插件唯一标识 |
| `plugins[].version` | string | 语义化版本号 |
| `plugins[].source` | string | 插件目录的相对路径 |
| `plugins[].description` | string | 插件描述 |
| `plugins[].category` | string | 分类 (如 "development") |
| `plugins[].tags` | array | 搜索标签 |
| `plugins[].author` | object | 作者信息 |
| `plugins[].lspServers` | object | (可选) 内联 LSP 配置 |

### 3.2 plugin.json - 插件元数据

**位置**: `<语言目录>/plugin.json`

**作用**: 定义单个插件的元数据信息。

**示例** (`rust-analyzer/plugin.json`):
```json
{
  "name": "rust-analyzer",
  "version": "0.1.0",
  "description": "Rust language server",
  "author": {
    "name": "Piebald LLC",
    "email": "support@piebald.ai"
  },
  "repository": "https://github.com/rust-lang/rust-analyzer",
  "license": "Apache-2.0 and MIT",
  "keywords": ["rust", "lsp", "language-server"]
}
```

**字段说明**:

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 插件名称，应与目录名一致 |
| `version` | string | 是 | 语义化版本号 |
| `description` | string | 是 | 简短描述 |
| `author` | object | 是 | 作者信息 (name, email) |
| `repository` | string | 否 | 上游仓库地址 |
| `license` | string | 否 | 许可证类型 |
| `keywords` | array | 否 | 关键词列表 |

### 3.3 .lsp.json - LSP 服务器配置

**位置**: `<语言目录>/.lsp.json`

**作用**: 定义 LSP 服务器的启动方式和语言映射。

#### 3.3.1 简单配置示例

**Rust** (`rust-analyzer/.lsp.json`):
```json
{
  "rust": {
    "command": "rust-analyzer",
    "args": [],
    "extensionToLanguage": {
      ".rs": "rust"
    },
    "transport": "stdio",
    "initializationOptions": {},
    "settings": {},
    "maxRestarts": 3
  }
}
```

**TypeScript/JavaScript** (`vtsls/.lsp.json`):
```json
{
  "typescript": {
    "command": "vtsls",
    "args": ["--stdio"],
    "extensionToLanguage": {
      ".ts": "typescript",
      ".tsx": "typescriptreact",
      ".js": "javascript",
      ".jsx": "javascriptreact",
      ".mjs": "javascript",
      ".cjs": "javascript"
    },
    "transport": "stdio",
    "initializationOptions": {},
    "settings": {},
    "maxRestarts": 3
  }
}
```

#### 3.3.2 多服务器配置示例

**HTML/CSS** (`vscode-langservers/.lsp.json`):
```json
{
  "html": {
    "command": "vscode-html-language-server",
    "args": ["--stdio"],
    "languages": ["html"],
    "fileExtensions": [".html", ".htm"],
    "transport": "stdio",
    "initializationOptions": {},
    "settings": {},
    "maxRestarts": 3
  },
  "css": {
    "command": "vscode-css-language-server",
    "args": ["--stdio"],
    "languages": ["css", "scss", "sass", "less"],
    "fileExtensions": [".css", ".scss", ".sass", ".less"],
    "transport": "stdio",
    "initializationOptions": {},
    "settings": {},
    "maxRestarts": 3
  }
}
```

#### 3.3.3 复杂初始化示例

**PowerShell** (`powershell-editor-services/.lsp.json`):
```json
{
  "powershell": {
    "command": "pwsh",
    "args": [
      "-NoLogo",
      "-NoProfile",
      "-Command",
      "if (-not (Get-Module -ListAvailable PowerShellEditorServices)) { Install-Module -Name PowerShellEditorServices -Scope CurrentUser -Force }; Import-Module PowerShellEditorServices; Start-EditorServices -HostName 'Claude Code' -HostProfileId 'ClaudeCode' -HostVersion '1.0.0' -Stdio -BundledModulesPath (Split-Path (Get-Module PowerShellEditorServices -ListAvailable).Path) -LogPath '/dev/null' -LogLevel 'None' -EnableConsoleRepl"
    ],
    "extensionToLanguage": {
      ".ps1": "powershell",
      ".psm1": "powershell",
      ".psd1": "powershell"
    },
    "transport": "stdio",
    "initializationOptions": {},
    "settings": {},
    "maxRestarts": 3
  }
}
```

**PowerShell 配置特点**:
- 使用 `pwsh` (PowerShell 7+) 作为启动命令
- 通过 `-Command` 参数执行复杂初始化脚本
- 自动检测并安装 PowerShellEditorServices 模块
- 配置 Host 信息和日志设置

#### 3.3.4 字段详解

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `command` | string | 是 | - | LSP 可执行文件名 (必须在 PATH 中) |
| `args` | array | 否 | `[]` | 命令行参数 |
| `extensionToLanguage` | object | 是* | - | 文件扩展名到 LSP 语言 ID 的映射 |
| `languages` | array | 否 | - | 支持的语言 ID 列表 (替代方案) |
| `fileExtensions` | array | 否 | - | 支持的文件扩展名列表 (替代方案) |
| `transport` | string | 是 | - | 通信方式，目前仅支持 `"stdio"` |
| `initializationOptions` | object | 否 | `{}` | LSP initialize 请求的选项 |
| `settings` | object | 否 | `{}` | LSP 服务器特定设置 |
| `maxRestarts` | number | 否 | `3` | 崩溃后最大重启次数 |

**语言映射方式**:
1. **extensionToLanguage** (推荐): 精确映射扩展名到语言 ID
2. **languages + fileExtensions**: 分别指定语言和扩展名列表

---

## 4. 插件详细配置

### 4.1 完整插件列表

| 插件目录 | 命令 | 文件扩展名 | 语言 ID |
|----------|------|-----------|---------|
| `vtsls` | `vtsls --stdio` | .ts, .tsx, .js, .jsx, .mjs, .cjs | typescript, typescriptreact, javascript, javascriptreact |
| `rust-analyzer` | `rust-analyzer` | .rs | rust |
| `pyright` | `pyright-langserver --stdio` | .py, .pyi, .pyw | python |
| `gopls` | `gopls` | .go | go |
| `jdtls` | `jdtls` | .java | java |
| `kotlin-lsp` | `kotlin-lsp` | .kt | kotlin |
| `clangd` | `clangd` | .c, .cpp, .cc, .cxx, .c++, .h, .hpp, .hh, .hxx, .h++ | c, cpp |
| `phpactor` | `phpactor language-server` | .php, .phtml, .php3, .php4, .php5, .phps | php |
| `ruby-lsp` | `ruby-lsp` | .rb | ruby |
| `omnisharp` | `omnisharp` | .cs | csharp |
| `powershell-editor-services` | `pwsh -Command "..."` | .ps1, .psm1, .psd1 | powershell |
| `vscode-langservers` | `vscode-html/css-language-server --stdio` | .html, .htm, .css, .scss, .sass, .less | html, css, scss, sass, less |
| `texlab` | `texlab` | .tex, .bib, .cls, .sty | latex, bibtex |
| `bsl-lsp` | `bsl-language-server` | .bsl, .os | bsl |

### 4.2 特殊配置说明

#### vtsls (TypeScript/JavaScript)
- 支持 6 种文件扩展名
- 区分 typescript/javascript 和 react 变体
- 需要全局安装: `npm install -g @vtsls/language-server typescript`

#### vscode-langservers (HTML/CSS)
- **唯一的多服务器插件**
- 包含 2 个独立的 LSP 服务器
- 需要安装: `npm install -g vscode-langservers-extracted`

#### powershell-editor-services
- **最复杂的初始化逻辑**
- 自动安装缺失的 PowerShellEditorServices 模块
- 需要 PowerShell 7+ (`pwsh`)

#### clangd (C/C++)
- 支持最多的文件扩展名 (10 种)
- 包含头文件和源文件变体
- LLVM 生态系统

---

## 5. Claude Code 集成

### 5.1 LSP 工具启用

Claude Code 2.0.30+ 开始支持 LSP，但需要启用:

```bash
# 设置环境变量启用 LSP 工具
export ENABLE_LSP_TOOL=1
```

### 5.2 插件安装流程

```bash
# 1. 添加市场
claude
/plugin marketplace add Piebald-AI/claude-code-lsps

# 2. 浏览并安装插件
/plugins
# 选择 "Browse and install plugins"
# 进入 "Claude Code Language Servers" 市场
# 用空格键选择需要的语言
# 按 "i" 安装
# 重启 Claude Code
```

### 5.3 LSP 操作能力

安装插件后，Claude 可执行以下操作:

| 操作 | LSP 方法 | 说明 |
|------|----------|------|
| 跳转定义 | `textDocument/definition` | 跳转到符号定义位置 |
| 悬停信息 | `textDocument/hover` | 显示符号的文档和类型信息 |
| 符号列表 | `textDocument/documentSymbol` | 列出文件中所有符号 |
| 查找引用 | `textDocument/references` | 查找符号的所有使用位置 |
| 工作区搜索 | `workspace/symbol` | 在整个工作区搜索符号 |

---

## 6. 实现原理

### 6.1 LSP 通信流程

```
Claude Code
    ↓
插件系统加载 .lsp.json
    ↓
启动 LSP 进程 (command + args)
    ↓
JSON-RPC 2.0 over stdio
    ↓
LSP 服务器 (rust-analyzer, gopls, etc.)
```

### 6.2 消息格式

LSP 使用 JSON-RPC 2.0 协议，通过 stdio 通信:

```
Content-Length: <length>\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"textDocument/definition",...}
```

### 6.3 初始化握手

1. 客户端发送 `initialize` 请求
2. 服务器返回能力声明
3. 客户端发送 `initialized` 通知
4. 开始正常请求/响应

---

## 7. 开发指南

### 7.1 添加新语言支持

1. **创建目录**: `mkdir <语言名>/`

2. **创建 plugin.json**:
```json
{
  "name": "<插件名>",
  "version": "0.1.0",
  "description": "<语言> language server",
  "author": { "name": "...", "email": "..." },
  "repository": "https://github.com/...",
  "license": "...",
  "keywords": ["<语言>", "lsp", "language-server"]
}
```

3. **创建 .lsp.json**:
```json
{
  "<语言ID>": {
    "command": "<可执行文件>",
    "args": ["--stdio"],
    "extensionToLanguage": {
      ".<扩展名>": "<语言ID>"
    },
    "transport": "stdio",
    "initializationOptions": {},
    "settings": {},
    "maxRestarts": 3
  }
}
```

4. **更新 marketplace.json**: 在 `plugins` 数组中添加新条目

5. **更新 README.md**: 添加安装说明

### 7.2 配置最佳实践

- **命令**: 使用不带路径的可执行文件名，依赖 PATH
- **参数**: 大多数 LSP 使用 `["--stdio"]`
- **扩展名映射**: 包含所有相关扩展名变体
- **maxRestarts**: 保持默认值 3，除非有特殊需求
- **目录命名**: 与 LSP 工具名保持一致

### 7.3 测试新插件

1. 确保 LSP 服务器已安装并在 PATH 中
2. 手动测试服务器: `<command> <args>`
3. 在 Claude Code 中安装插件
4. 打开对应语言的文件
5. 测试各项 LSP 操作

---

## 8. 技术细节

### 8.1 transport 选项

目前所有插件都使用 `"stdio"`:
- 通过标准输入/输出通信
- 简单可靠
- 跨平台兼容

理论上 LSP 还支持:
- TCP socket
- Named pipes
- HTTP

但 Claude Code 插件系统目前仅实现 stdio。

### 8.2 语言 ID 标准

语言 ID 需要与 LSP 规范一致:

| 语言 | 标准 ID |
|------|---------|
| TypeScript | `typescript` |
| TypeScript (React) | `typescriptreact` |
| JavaScript | `javascript` |
| JavaScript (React) | `javascriptreact` |
| Python | `python` |
| Rust | `rust` |
| Go | `go` |
| Java | `java` |
| C | `c` |
| C++ | `cpp` |
| C# | `csharp` |
| Ruby | `ruby` |
| PHP | `php` |
| PowerShell | `powershell` |
| HTML | `html` |
| CSS | `css` |
| SCSS | `scss` |
| LaTeX | `latex` |
| BibTeX | `bibtex` |

### 8.3 重启策略

`maxRestarts` 控制服务器崩溃后的重启次数:
- 默认值: 3
- 超过限制后停止重启，需手动干预
- 用于防止崩溃循环

---

## 9. 局限性与注意事项

### 9.1 当前限制

- **Beta 状态**: Claude Code LSP 支持仍在早期阶段
- **无 UI 指示**: 无法直观看到 LSP 服务器状态
- **需要 Patch**: 完整功能需要使用 [tweakcc](https://github.com/Piebald-AI/tweakcc)
- **仅 stdio**: 不支持其他传输协议

### 9.2 常见问题

1. **LSP 服务器未找到**: 确保可执行文件在 PATH 中
2. **插件未生效**: 重启 Claude Code
3. **功能不工作**: 检查 `$ENABLE_LSP_TOOL=1` 是否设置
4. **特定语言问题**: 参考各语言的安装说明

### 9.3 调试建议

- 检查 LSP 服务器是否可独立运行
- 验证文件扩展名映射是否正确
- 查看 Claude Code 日志

---

## 10. 参考资源

- **项目仓库**: https://github.com/Piebald-AI/claude-code-lsps
- **LSP 规范**: https://microsoft.github.io/language-server-protocol
- **Claude Code 文档**: https://code.claude.com/docs
- **tweakcc (补丁工具)**: https://github.com/Piebald-AI/tweakcc
- **Claude Code 插件市场文档**: https://code.claude.com/docs/en/plugin-marketplaces

---

## 11. 文件清单

```
claude-code-lsps/
├── .claude-plugin/marketplace.json   # 市场清单 (262 行)
├── CLAUDE.md                         # 开发指南 (72 行)
├── README.md                         # 用户文档 (307 行)
├── bsl-lsp/
│   ├── plugin.json
│   └── .lsp.json
├── clangd/
│   ├── plugin.json
│   └── .lsp.json
├── gopls/
│   ├── plugin.json
│   └── .lsp.json
├── jdtls/
│   ├── plugin.json
│   └── .lsp.json
├── kotlin-lsp/
│   ├── plugin.json
│   └── .lsp.json
├── omnisharp/
│   ├── plugin.json
│   └── .lsp.json
├── phpactor/
│   ├── plugin.json
│   └── .lsp.json
├── powershell-editor-services/
│   ├── plugin.json
│   └── .lsp.json
├── pyright/
│   ├── plugin.json
│   └── .lsp.json
├── ruby-lsp/
│   ├── plugin.json
│   └── .lsp.json
├── rust-analyzer/
│   ├── plugin.json
│   └── .lsp.json
├── texlab/
│   ├── plugin.json
│   └── .lsp.json
├── vscode-langservers/
│   ├── plugin.json
│   └── .lsp.json
└── vtsls/
    ├── plugin.json
    └── .lsp.json
```

**总计**: 31 个文件 (3 个文档 + 14 个插件目录 × 2 个配置文件)
