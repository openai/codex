# OpenCode LSP 支持设计与实现分析

本文档详细分析 opencode 如何实现 LSP (Language Server Protocol) 支持，供 codex 研发参考。

---

## 目录

1. [架构概览](#1-架构概览)
2. [配置系统](#2-配置系统)
3. [LSP Tool 实现](#3-lsp-tool-实现)
4. [内置 LSP Server 注册表](#4-内置-lsp-server-注册表)
5. [Server 启动与生命周期管理](#5-server-启动与生命周期管理)
6. [LSP Client 实现](#6-lsp-client-实现)
7. [自动下载机制](#7-自动下载机制)
8. [关键代码路径](#8-关键代码路径)

---

## 1. 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                        OpenCode LSP 架构                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │   LSP Tool   │───▶│  LSP Client  │───▶│  LSP Server      │  │
│  │  (lsp.ts)    │    │ (client.ts)  │    │  (child process) │  │
│  └──────────────┘    └──────────────┘    └──────────────────┘  │
│         │                   │                     │             │
│         │                   │                     │             │
│         ▼                   ▼                     ▼             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │ Tool Registry│    │ JSON-RPC 2.0 │    │  Server Registry │  │
│  │(registry.ts) │    │(vscode-jsonrpc)   │  (server.ts)     │  │
│  └──────────────┘    └──────────────┘    └──────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Config System                          │  │
│  │  Global → Project → CLI Flags → User Custom               │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**核心组件:**

| 组件 | 文件 | 职责 |
|------|------|------|
| LSP Tool | `src/tool/lsp.ts` | 向 AI Agent 暴露 LSP 操作接口 |
| LSP Namespace | `src/lsp/index.ts` | 协调 Server/Client，执行 LSP 请求 |
| LSP Client | `src/lsp/client.ts` | JSON-RPC 通信，管理单个 Server 连接 |
| LSP Server | `src/lsp/server.ts` | 40+ 内置 Server 定义与启动逻辑 |
| Tool Registry | `src/tool/registry.ts` | Tool 注册与实验性 Flag 控制 |
| Config | `src/config/config.ts` | LSP 配置 Schema 与合并逻辑 |

---

## 2. 配置系统

### 2.1 配置 Schema

**文件:** `packages/opencode/src/config/config.ts:748-783`

```typescript
lsp: z
  .union([
    z.literal(false),                    // 全局禁用 LSP
    z.record(
      z.string(),                        // Server ID 或自定义名称
      z.union([
        z.object({
          disabled: z.literal(true),     // 禁用特定 Server
        }),
        z.object({
          command: z.array(z.string()),           // 启动命令
          extensions: z.array(z.string()).optional(),  // 文件扩展名
          disabled: z.boolean().optional(),
          env: z.record(z.string(), z.string()).optional(),  // 环境变量
          initialization: z.record(z.string(), z.any()).optional(),  // LSP InitializeParams
        }),
      ]),
    ),
  ])
  .optional()
```

### 2.2 配置示例

```jsonc
// opencode.json
{
  "$schema": "https://opencode.ai/config.json",

  // 方式1: 通过 tools 启用 LSP Tool (需要实验性 Flag)
  "tools": {
    "lsp": true
  },

  // 方式2: 配置 LSP Servers
  "lsp": {
    // 禁用特定 Server
    "pyright": { "disabled": true },

    // 覆盖内置 Server 配置
    "typescript": {
      "extensions": [".ts", ".tsx", ".js"],
      "initialization": {
        "tsserver": {
          "path": "/custom/path/to/tsserver.js"
        }
      }
    },

    // 添加自定义 Server
    "my-custom-lsp": {
      "command": ["my-lsp-server", "--stdio"],
      "extensions": [".custom", ".mycustom"],
      "env": { "CUSTOM_VAR": "value" },
      "initialization": { "customOption": true }
    }
  }
}
```

### 2.3 配置合并优先级

```
1. 全局默认配置 (~/.opencode/opencode.json)
2. OPENCODE_CONFIG 环境变量指定的配置
3. 项目目录向上搜索 opencode.jsonc / opencode.json
4. OPENCODE_CONFIG_CONTENT 环境变量
5. Well-known Server 配置
6. .opencode/ 目录配置
```

### 2.4 实验性 Flag 控制

**文件:** `src/flag/flag.ts:33`

```typescript
export const OPENCODE_EXPERIMENTAL_LSP_TOOL =
  OPENCODE_EXPERIMENTAL || truthy("OPENCODE_EXPERIMENTAL_LSP_TOOL")
```

**启用方式:**
- `OPENCODE_EXPERIMENTAL_LSP_TOOL=true`
- `OPENCODE_EXPERIMENTAL=true` (启用所有实验性功能)

---

## 3. LSP Tool 实现

### 3.1 Tool 定义

**文件:** `packages/opencode/src/tool/lsp.ts:21-87`

```typescript
const operations = [
  "goToDefinition",
  "findReferences",
  "hover",
  "documentSymbol",
  "workspaceSymbol",
  "goToImplementation",
  "prepareCallHierarchy",
  "incomingCalls",
  "outgoingCalls",
] as const

export const LspTool = Tool.define("lsp", {
  description: DESCRIPTION,  // 从 lsp.txt 加载
  parameters: z.object({
    operation: z.enum(operations),
    filePath: z.string(),
    line: z.number().int().min(1),        // 1-based (编辑器显示)
    character: z.number().int().min(1),   // 1-based (编辑器显示)
  }),
  async execute(args) {
    // 转换为 0-based (LSP 协议)
    const position = {
      line: args.line - 1,
      character: args.character - 1,
    }

    // 解析文件路径
    const file = path.isAbsolute(args.filePath)
      ? args.filePath
      : path.join(Instance.directory, args.filePath)

    // 根据 operation 调用对应的 LSP 方法
    switch (args.operation) {
      case "goToDefinition":
        return LSP.definition({ file, position })
      case "findReferences":
        return LSP.references({ file, position })
      // ... 其他 operations
    }
  },
})
```

### 3.2 Tool 注册

**文件:** `packages/opencode/src/tool/registry.ts:107`

```typescript
async function all(): Promise<Tool.Info[]> {
  return [
    InvalidTool,
    BashTool,
    ReadTool,
    GlobTool,
    GrepTool,
    EditTool,
    WriteTool,
    TaskTool,
    WebFetchTool,
    TodoWriteTool,
    TodoReadTool,
    WebSearchTool,
    CodeSearchTool,
    SkillTool,
    // LSP Tool 条件注册
    ...(Flag.OPENCODE_EXPERIMENTAL_LSP_TOOL ? [LspTool] : []),
    ...(config.experimental?.batch_tool === true ? [BatchTool] : []),
    ...custom,
  ]
}
```

### 3.3 支持的 LSP Operations

| Operation | LSP Method | 功能描述 |
|-----------|-----------|----------|
| `goToDefinition` | `textDocument/definition` | 跳转到符号定义 |
| `findReferences` | `textDocument/references` | 查找所有引用 |
| `hover` | `textDocument/hover` | 获取悬停信息(文档/类型) |
| `documentSymbol` | `textDocument/documentSymbol` | 获取文档所有符号 |
| `workspaceSymbol` | `workspace/symbol` | 跨工作区搜索符号 |
| `goToImplementation` | `textDocument/implementation` | 跳转到实现 |
| `prepareCallHierarchy` | `textDocument/prepareCallHierarchy` | 准备调用层次 |
| `incomingCalls` | `callHierarchy/incomingCalls` | 查找调用者 |
| `outgoingCalls` | `callHierarchy/outgoingCalls` | 查找被调用者 |

---

## 4. 内置 LSP Server 注册表

### 4.1 Server 接口定义

**文件:** `packages/opencode/src/lsp/server.ts:1-13`

```typescript
export interface Info {
  id: string                                // 唯一标识符
  extensions: string[]                      // 处理的文件扩展名
  global?: boolean                          // 是否全局作用域
  root: RootFunction                        // 项目根目录查找函数
  spawn(root: string): Promise<Handle | undefined>  // 启动进程
}

export interface Handle {
  process: ChildProcessWithoutNullStreams   // Node 子进程
  initialization?: Record<string, any>      // LSP InitializeParams
}

type RootFunction = (file: string) => Promise<string | undefined>
```

### 4.2 内置 Server 列表 (40+)

**文件:** `packages/opencode/src/lsp/server.ts` (1913 行)

| Server | ID | 文件扩展名 | 根目录检测 | 自动安装 |
|--------|-----|-----------|-----------|---------|
| **Deno** | `deno` | `.ts .tsx .js .jsx .mjs` | `deno.json` | 需预装 |
| **TypeScript** | `typescript` | `.ts .tsx .js .jsx .mjs .cjs .mts .cts` | `package-lock.json` `.bun.lockb` | bun install |
| **Vue** | `vue` | `.vue` | package manager lock files | npm install |
| **ESLint** | `eslint` | `.ts .tsx .js .jsx .vue` | lock files | GitHub 下载 |
| **Biome** | `biome` | `.ts .tsx .js .jsx .json .vue .css` | `biome.json` | 需预装 |
| **Gopls** | `gopls` | `.go` | `go.mod` `go.sum` `go.work` | `go install` |
| **Pyright** | `pyright` | `.py .pyi` | `pyproject.toml` | npm install |
| **Ty** (实验性) | `ty` | `.py .pyi` | config files | 需预装 |
| **Ruby-LSP** | `ruby-lsp` | `.rb .rake .gemspec .ru` | `Gemfile` | `gem install` |
| **Rust-Analyzer** | `rust` | `.rs` | `Cargo.toml` (workspace root) | 需预装 |
| **Clangd** | `clangd` | `.c .cpp .cc .h .hpp` | CMake/config files | GitHub 下载 |
| **JDTLS** | `jdtls` | `.java` | `pom.xml` `build.gradle` | GitHub 下载 |
| **Zls** | `zls` | `.zig .zon` | `build.zig` | GitHub 下载 |
| **CSharp** | `csharp` | `.cs` | `.sln` `.csproj` | `dotnet tool` |
| **Svelte** | `svelte` | `.svelte` | lock files | npm install |
| **Astro** | `astro` | `.astro` | lock files | npm install |
| **YamlLS** | `yaml-ls` | `.yaml .yml` | lock files | npm install |
| **LuaLS** | `lua-ls` | `.lua` | `.luarc.json` | GitHub 下载 |
| **BashLS** | `bash` | `.sh .bash .zsh .ksh` | - | npm install |
| **TerraformLS** | `terraform` | `.tf .tfvars` | `.terraform.lock.hcl` | GitHub 下载 |
| **Dart** | `dart` | `.dart` | `pubspec.yaml` | 需预装 |
| **Gleam** | `gleam` | `.gleam` | `gleam.toml` | 需预装 |
| **Clojure** | `clojure-lsp` | `.clj .cljs .cljc .edn` | `deps.edn` | 需预装 |
| **Nixd** | `nixd` | `.nix` | `flake.nix` | 需预装 |
| **HLS** | `haskell-language-server` | `.hs .lhs` | `stack.yaml` `cabal.project` | 需预装 |
| ... | ... | ... | ... | ... |

### 4.3 根目录查找策略

**文件:** `packages/opencode/src/lsp/server.ts:24-45`

```typescript
const NearestRoot = (
  includePatterns: string[],
  excludePatterns?: string[]
): RootFunction => {
  return async (file) => {
    // 1. 检查排除模式 (如 deno.json)
    if (excludePatterns) {
      const excluded = await Filesystem.up({
        targets: excludePatterns,
        start: path.dirname(file),
        stop: Instance.directory,
      }).next()
      if (excluded.value) return undefined
    }

    // 2. 向上查找包含模式 (如 package.json)
    const files = Filesystem.up({
      targets: includePatterns,
      start: path.dirname(file),
      stop: Instance.directory,
    })
    const first = await files.next()
    if (!first.value) return Instance.directory
    return path.dirname(first.value)
  }
}
```

**示例:**
- **TypeScript**: 查找 `package-lock.json` → `.bun.lockb`，排除 `deno.json`
- **Go**: 优先查找 `go.work`，然后 `go.mod` / `go.sum`
- **Rust**: 查找包含 `[workspace]` 的 `Cargo.toml`

---

## 5. Server 启动与生命周期管理

### 5.1 Server 启动流程

**文件:** `packages/opencode/src/lsp/index.ts:177-262`

```typescript
export async function getClients(file: string): Promise<LSPClient.Info[]> {
  const s = await state()
  const ext = path.extname(file)
  const results: LSPClient.Info[] = []

  for (const server of Object.values(s.servers)) {
    // 1. 检查扩展名匹配
    if (!server.extensions.includes(ext)) continue

    // 2. 查找项目根目录
    const root = await server.root(file)
    if (!root) continue

    // 3. 检查是否已标记为失败
    const key = `${server.id}:${root}`
    if (s.broken.has(key)) continue

    // 4. 复用已存在的 Client
    const existing = s.clients.find(
      c => c.serverID === server.id && c.root === root
    )
    if (existing) {
      results.push(existing)
      continue
    }

    // 5. 检查是否正在启动中 (防止重复启动)
    const spawning = s.spawning.get(key)
    if (spawning) {
      const client = await spawning
      if (client) results.push(client)
      continue
    }

    // 6. 启动新 Server
    const promise = (async () => {
      const handle = await server.spawn(root)
      if (!handle) {
        s.broken.add(key)
        return undefined
      }

      const client = await LSPClient.create({
        root,
        serverID: server.id,
        server: handle,
      })
      s.clients.push(client)
      return client
    })()

    s.spawning.set(key, promise)
    const client = await promise
    s.spawning.delete(key)
    if (client) results.push(client)
  }

  // 7. 去重 (同一 root+serverID)
  return [...new Map(results.map(c => [`${c.root}:${c.serverID}`, c])).values()]
}
```

### 5.2 Server Spawn 实现示例 (Gopls)

**文件:** `packages/opencode/src/lsp/server.ts:360-391`

```typescript
export const Gopls: Info = {
  id: "gopls",
  extensions: [".go"],
  root: NearestRoot(["go.work", "go.mod", "go.sum"]),

  async spawn(root) {
    // 1. 查找已安装的 gopls
    let bin = Bun.which("gopls", {
      PATH: process.env["PATH"] + path.delimiter + Global.Path.bin,
    })

    // 2. 如果未找到，尝试自动安装
    if (!bin) {
      if (!Bun.which("go")) return  // 需要 go 命令
      if (Flag.OPENCODE_DISABLE_LSP_DOWNLOAD) return

      const proc = Bun.spawn({
        cmd: ["go", "install", "golang.org/x/tools/gopls@latest"],
        env: { ...process.env, GOBIN: Global.Path.bin },
        stdout: "pipe",
        stderr: "pipe",
        stdin: "pipe",
      })

      const exit = await proc.exited
      if (exit !== 0) {
        log.error("Failed to install gopls")
        return
      }

      bin = path.join(
        Global.Path.bin,
        "gopls" + (process.platform === "win32" ? ".exe" : "")
      )
    }

    // 3. 启动进程
    return {
      process: spawn(bin!, {
        cwd: root,
      }),
    }
  },
}
```

### 5.3 State 管理

**文件:** `packages/opencode/src/lsp/index.ts:79-144`

```typescript
const state = Instance.state(
  async () => {
    const cfg = await Config.get()

    // 1. 加载所有内置 Servers
    const servers: Record<string, LSPServer.Info> = {}
    for (const server of Object.values(LSPServer)) {
      servers[server.id] = server
    }

    // 2. 处理实验性 Servers (Ty vs Pyright)
    if (Flag.OPENCODE_EXPERIMENTAL_LSP_TY) {
      delete servers["pyright"]
    } else {
      delete servers["ty"]
    }

    // 3. 应用用户配置覆盖
    for (const [name, item] of Object.entries(cfg.lsp ?? {})) {
      if (item.disabled) {
        delete servers[name]
        continue
      }

      // 自定义 Server 或覆盖内置
      servers[name] = {
        ...servers[name],
        id: name,
        extensions: item.extensions ?? servers[name]?.extensions ?? [],
        spawn: async (root) => ({
          process: spawn(item.command[0], item.command.slice(1), {
            cwd: root,
            env: { ...process.env, ...item.env },
          }),
          initialization: item.initialization,
        }),
      }
    }

    return {
      broken: new Set<string>(),           // 失败的 Server
      servers,                             // Server 注册表
      clients: [] as LSPClient.Info[],     // 活跃的 Client
      spawning: new Map<string, Promise<LSPClient.Info | undefined>>(),
    }
  },

  // Shutdown 回调
  async (state) => {
    await Promise.all(state.clients.map(c => c.shutdown()))
  }
)
```

---

## 6. LSP Client 实现

### 6.1 Client 创建与初始化

**文件:** `packages/opencode/src/lsp/client.ts:42-130`

```typescript
export async function create(input: {
  root: string
  serverID: string
  server: LSPServer.Handle
}): Promise<Info> {
  // 1. 创建 JSON-RPC 连接
  const connection = createMessageConnection(
    new StreamMessageReader(input.server.process.stdout),
    new StreamMessageWriter(input.server.process.stdin)
  )

  // 2. 监听通知
  connection.onNotification("textDocument/publishDiagnostics", (params) => {
    // 处理诊断信息 (150ms debounce)
    debouncedDiagnostics(params)
  })

  // 3. 处理请求
  connection.onRequest("workspace/workspaceFolders", () => [{
    name: "workspace",
    uri: pathToFileURL(input.root).href,
  }])

  connection.onRequest("workspace/configuration", () =>
    [input.server.initialization ?? {}]
  )

  connection.onRequest("client/registerCapability", () => {})
  connection.onRequest("client/unregisterCapability", () => {})

  // 4. 开始监听
  connection.listen()

  // 5. 发送 initialize 请求
  await withTimeout(
    connection.sendRequest("initialize", {
      rootUri: pathToFileURL(input.root).href,
      processId: input.server.process.pid,
      workspaceFolders: [{
        name: "workspace",
        uri: pathToFileURL(input.root).href,
      }],
      initializationOptions: input.server.initialization,
      capabilities: {
        window: { workDoneProgress: true },
        workspace: { configuration: true },
        textDocument: {
          synchronization: { didOpen: true, didChange: true },
          publishDiagnostics: { versionSupport: true },
        },
      },
    }),
    45_000  // 45 秒超时
  )

  // 6. 发送 initialized 通知
  connection.sendNotification("initialized")

  // 7. 发送配置变更
  connection.sendNotification("workspace/didChangeConfiguration", {
    settings: input.server.initialization ?? {},
  })

  return {
    root: input.root,
    serverID: input.serverID,
    connection,
    notify: { open: ... },
    diagnostics: new Map(),
    waitForDiagnostics: ...,
    shutdown: ...,
  }
}
```

### 6.2 文件同步

**文件:** `packages/opencode/src/lsp/client.ts:132-170`

```typescript
const versions = new Map<string, number>()

notify: {
  async open(input: { path: string }) {
    const uri = pathToFileURL(input.path).href
    const version = (versions.get(uri) ?? 0) + 1
    versions.set(uri, version)

    const content = await Bun.file(input.path).text()
    const languageId = LANGUAGE_EXTENSIONS[path.extname(input.path)] ?? "plaintext"

    if (version === 1) {
      // 首次打开
      connection.sendNotification("textDocument/didOpen", {
        textDocument: {
          uri,
          languageId,
          version,
          text: content,
        },
      })
    } else {
      // 后续变更
      connection.sendNotification("textDocument/didChange", {
        textDocument: { uri, version },
        contentChanges: [{ text: content }],
      })
    }
  },
}
```

### 6.3 LSP 请求方法

**文件:** `packages/opencode/src/lsp/index.ts:303-455`

```typescript
// Hover
export async function hover(input: {
  file: string
  position: Position
}): Promise<Hover | null> {
  const clients = await getClients(input.file)
  for (const client of clients) {
    await client.notify.open({ path: input.file })
    const result = await client.connection.sendRequest("textDocument/hover", {
      textDocument: { uri: pathToFileURL(input.file).href },
      position: input.position,
    }).catch(() => null)
    if (result) return result
  }
  return null
}

// Definition
export async function definition(input: {
  file: string
  position: Position
}): Promise<Location[]> {
  const clients = await getClients(input.file)
  for (const client of clients) {
    await client.notify.open({ path: input.file })
    const result = await client.connection.sendRequest("textDocument/definition", {
      textDocument: { uri: pathToFileURL(input.file).href },
      position: input.position,
    }).catch(() => [])
    if (result?.length) return result
  }
  return []
}

// Workspace Symbol (带过滤)
export async function workspaceSymbol(query: string): Promise<SymbolInformation[]> {
  const clients = await state().then(s => s.clients)
  const results: SymbolInformation[] = []

  const kinds = [
    SymbolKind.Class, SymbolKind.Function, SymbolKind.Method,
    SymbolKind.Interface, SymbolKind.Variable, SymbolKind.Constant,
    SymbolKind.Struct, SymbolKind.Enum,
  ]

  for (const client of clients) {
    const symbols = await client.connection.sendRequest("workspace/symbol", { query })
      .catch(() => [])

    for (const symbol of symbols) {
      if (kinds.includes(symbol.kind)) {
        results.push(symbol)
        if (results.length >= 10) return results  // 限制 10 个
      }
    }
  }
  return results
}

// Call Hierarchy
export async function incomingCalls(input: {
  file: string
  position: Position
}): Promise<CallHierarchyIncomingCall[]> {
  // 先获取 CallHierarchyItem
  const items = await prepareCallHierarchy(input)
  if (!items.length) return []

  const clients = await getClients(input.file)
  for (const client of clients) {
    const result = await client.connection.sendRequest(
      "callHierarchy/incomingCalls",
      { item: items[0] }
    ).catch(() => [])
    if (result?.length) return result
  }
  return []
}
```

---

## 7. 自动下载机制

### 7.1 下载策略

OpenCode 支持多种自动安装方式:

| 方式 | 示例 Server | 命令 |
|------|------------|------|
| 语言包管理器 | Gopls | `go install golang.org/x/tools/gopls@latest` |
| npm/bun | TypeScript, YamlLS | `bun install typescript-language-server` |
| gem | Ruby-LSP | `gem install ruby-lsp` |
| dotnet | CSharp, FSharp | `dotnet tool install csharp-ls` |
| GitHub Releases | Clangd, Zls, JDTLS | 下载预编译二进制 |

### 7.2 GitHub 下载实现示例 (Clangd)

**文件:** `packages/opencode/src/lsp/server.ts:929-1023`

```typescript
async spawn(root) {
  let bin = Bun.which("clangd", {
    PATH: process.env["PATH"] + path.delimiter + Global.Path.bin,
  })

  if (!bin) {
    if (Flag.OPENCODE_DISABLE_LSP_DOWNLOAD) return

    // 1. 确定平台
    const platform = process.platform === "darwin" ? "mac"
      : process.platform === "linux" ? "linux"
      : process.platform === "win32" ? "windows" : null
    if (!platform) return

    const arch = process.arch === "arm64" ? "aarch64" : "x86_64"

    // 2. 获取最新 Release
    const releases = await fetch(
      "https://api.github.com/repos/clangd/clangd/releases/latest"
    ).then(r => r.json())

    // 3. 下载对应平台的包
    const asset = releases.assets.find(a =>
      a.name.includes(platform) && a.name.includes(arch)
    )

    const zipPath = path.join(Global.Path.tmp, asset.name)
    const response = await fetch(asset.browser_download_url)
    await Bun.write(zipPath, response)

    // 4. 解压
    await Bun.spawn({
      cmd: ["unzip", "-o", zipPath, "-d", Global.Path.bin],
    }).exited

    bin = path.join(Global.Path.bin, "clangd_xxx", "bin", "clangd")
  }

  return { process: spawn(bin, { cwd: root }) }
}
```

### 7.3 禁用自动下载

设置环境变量 `OPENCODE_DISABLE_LSP_DOWNLOAD=true` 可禁用所有自动下载。

---

## 8. 关键代码路径

### 8.1 文件路径索引

| 功能 | 文件路径 | 关键行号 |
|------|---------|---------|
| **LSP Tool 定义** | `src/tool/lsp.ts` | 21-87 |
| **Tool 注册** | `src/tool/registry.ts` | 107 |
| **实验性 Flag** | `src/flag/flag.ts` | 33 |
| **配置 Schema** | `src/config/config.ts` | 748-783 |
| **LSP Namespace** | `src/lsp/index.ts` | 1-485 |
| **LSP Client** | `src/lsp/client.ts` | 1-229 |
| **LSP Server 注册表** | `src/lsp/server.ts` | 1-1913 |
| **语言扩展名映射** | `src/lsp/language.ts` | 1-117 |
| **Tool 描述** | `src/tool/lsp.txt` | - |
| **调试命令** | `src/cli/cmd/debug/lsp.ts` | - |
| **测试** | `test/lsp/client.test.ts` | - |

### 8.2 数据流

```
用户请求 (goToDefinition)
    │
    ▼
LSP Tool (lsp.ts:execute)
    │ 参数: { operation, filePath, line, character }
    │ 转换: 1-based → 0-based
    ▼
LSP Namespace (index.ts:definition)
    │ 获取适用的 Clients
    ▼
getClients(file) (index.ts:177-262)
    │ 1. 匹配扩展名
    │ 2. 查找项目根目录
    │ 3. 复用/启动 Server
    ▼
LSPClient.create() (client.ts:42-130)
    │ 1. 建立 JSON-RPC 连接
    │ 2. 发送 initialize 请求
    │ 3. 注册通知处理器
    ▼
connection.sendRequest() (vscode-jsonrpc)
    │ textDocument/definition
    ▼
LSP Server 进程 (子进程)
    │ 处理请求，返回结果
    ▼
返回给 AI Agent
```

---

## 9. 设计要点总结

### 9.1 核心设计模式

1. **按需启动 (Lazy Spawning)**: Server 仅在首次需要时启动
2. **连接复用 (Connection Reuse)**: 同一 root+serverID 复用单个 Client
3. **优雅降级 (Graceful Degradation)**: 失败的 Server 被标记，不阻塞后续操作
4. **自动安装 (Auto Install)**: 20+ Server 支持自动下载安装
5. **可扩展 (Extensible)**: 用户可通过配置添加自定义 Server
6. **实验性控制 (Feature Flag)**: LSP Tool 默认禁用，需显式启用

### 9.2 关键实现细节

- **JSON-RPC 通信**: 使用 `vscode-jsonrpc` 库
- **进程管理**: Node.js `child_process.spawn`
- **超时控制**: 初始化 45s，诊断等待 3s
- **诊断去抖**: 150ms debounce 允许语义分析完成
- **符号过滤**: workspaceSymbol 只返回重要类型，限制 10 个结果
- **文件版本**: 跟踪文件版本支持增量更新

### 9.3 codex 实现建议

如果要在 codex 中实现类似功能:

1. **Rust 化重写**: 使用 `tower-lsp` 或手动实现 JSON-RPC
2. **异步运行时**: 使用 Tokio 管理 Server 进程
3. **配置集成**: 扩展现有 `Config` 结构体
4. **Tool 注册**: 参考现有 Tool 模式 (`spec.rs` / `spec_ext.rs`)
5. **实验性控制**: 参考 `features.rs` 添加 Feature Flag

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
