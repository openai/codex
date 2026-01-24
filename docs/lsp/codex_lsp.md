# Codex LSP Tool 设计与实现文档

> 基于 OpenCode、Claude Code、cclsp 最佳实践设计

---

## 目录

1. [概述](#1-概述)
2. [架构设计](#2-架构设计)
3. [核心组件](#3-核心组件)
4. [LSP Tool 规范](#4-lsp-tool-规范)
5. [配置系统](#5-配置系统)
6. [文件同步机制](#6-文件同步机制)
7. [符号解析策略](#7-符号解析策略)
8. [诊断系统集成](#8-诊断系统集成)
9. [服务器生命周期管理](#9-服务器生命周期管理)
10. [错误处理](#10-错误处理)
11. [测试策略](#11-测试策略)
12. [实现路径](#12-实现路径)
13. [参考实现对比](#13-参考实现对比)

---

## 1. 概述

### 1.1 什么是 Codex LSP Tool

Codex LSP Tool 是 codex-rs 的内置代码智能工具，通过 Language Server Protocol (LSP) 提供：

- **Go to Definition** - 跳转到符号定义
- **Find References** - 查找所有引用
- **Hover** - 获取类型信息和文档
- **Document Symbols** - 获取文件内所有符号
- **Diagnostics** - 获取错误和警告

### 1.2 设计目标

| 目标 | 描述 |
|------|------|
| **LLM 友好** | 使用符号名称 + 类型，而非精确行列号 |
| **最小侵入** | 遵循 `*_ext.rs` 扩展模式，减少上游合并冲突 |
| **预装优先** | 不自动下载，要求用户预装 LSP Server |
| **诊断集成** | 自动推送诊断到对话上下文 |
| **按需同步** | On-demand 文件同步 + didChangeWatchedFiles |

### 1.3 支持的语言

| 语言 | LSP Server | 文件扩展名 | 安装命令 |
|------|------------|-----------|----------|
| Rust | rust-analyzer | `.rs` | `rustup component add rust-analyzer` |
| Go | gopls | `.go` | `go install golang.org/x/tools/gopls@latest` |
| Python | pyright | `.py`, `.pyi` | `npm install -g pyright` |

### 1.4 启用方式

```toml
# ~/.codex/config.toml
[features]
lsp = true

[lsp]
enabled = true
```

或通过环境变量：

```bash
CODEX_FEATURE_LSP=true codex
```

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Codex CLI / TUI                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                        Core Layer                                 │   │
│  │  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐   │   │
│  │  │  LSP Tool    │    │  Tool        │    │  Diagnostics     │   │   │
│  │  │  Handler     │───▶│  Registry    │    │  Attachment Gen  │   │   │
│  │  │  (ext/lsp.rs)│    │              │    │  (system_reminder)│   │   │
│  │  └──────────────┘    └──────────────┘    └──────────────────┘   │   │
│  │         │                                         ▲              │   │
│  │         │                                         │              │   │
│  │         ▼                                         │              │   │
│  │  ┌────────────────────────────────────────────────┴──────────┐   │   │
│  │  │                    codex-lsp Crate                         │   │   │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │   │   │
│  │  │  │ LspServer    │  │  LspClient   │  │  Diagnostics     │ │   │   │
│  │  │  │ Manager      │──│              │──│  Store           │ │   │   │
│  │  │  │              │  │              │  │  (debounced)     │ │   │   │
│  │  │  └──────────────┘  └──────────────┘  └──────────────────┘ │   │   │
│  │  │         │                 │                               │   │   │
│  │  │         │                 ▼                               │   │   │
│  │  │         │          ┌──────────────┐                       │   │   │
│  │  │         │          │  Symbol      │                       │   │   │
│  │  │         │          │  Resolver    │                       │   │   │
│  │  │         │          │  (name+kind) │                       │   │   │
│  │  │         │          └──────────────┘                       │   │   │
│  │  │         │                 │                               │   │   │
│  │  │         ▼                 ▼                               │   │   │
│  │  │  ┌──────────────────────────────────────────────────────┐ │   │   │
│  │  │  │              JSON-RPC 2.0 Protocol                    │ │   │   │
│  │  │  │              (Content-Length framing)                 │ │   │   │
│  │  │  └──────────────────────────────────────────────────────┘ │   │   │
│  │  └───────────────────────────────────────────────────────────┘   │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                    │                                     │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     │ stdio
              ┌──────────────────────┼──────────────────────┐
              │                      │                      │
              ▼                      ▼                      ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│   rust-analyzer     │ │       gopls         │ │      pyright        │
│   (Rust LSP)        │ │     (Go LSP)        │ │   (Python LSP)      │
└─────────────────────┘ └─────────────────────┘ └─────────────────────┘
```

### 2.2 Crate 结构

```
codex-rs/
├── lsp/                              # 新建 crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                    # 公共 API 导出
│       ├── error.rs                  # LspErr 错误类型
│       ├── config.rs                 # LspConfig, BuiltinServer
│       ├── symbols.rs                # 符号解析 (name + kind)
│       ├── protocol.rs               # JSON-RPC 2.0 实现
│       ├── client.rs                 # LspClient - 与单个 Server 通信
│       ├── server.rs                 # LspServerManager - 生命周期管理
│       └── diagnostics.rs            # DiagnosticsStore - 诊断缓存
│
├── core/src/
│   ├── tools/
│   │   └── ext/
│   │       └── lsp.rs                # Tool Spec 定义 (NEW)
│   ├── tools/handlers/ext/
│   │   └── lsp.rs                    # LspHandler 实现 (NEW)
│   ├── tools/spec_ext.rs             # 添加 register_lsp() (MODIFY)
│   ├── features.rs                   # 添加 Feature::Lsp (MODIFY)
│   └── features_ext.rs               # 添加 FeatureSpec (MODIFY)
│
└── protocol/src/
    └── config_types_ext.rs           # LspConfig 类型 (MODIFY)
```

### 2.3 组件职责

| 组件 | 文件 | 职责 |
|------|------|------|
| **LspServerManager** | `server.rs` | 管理多个 LSP Server 进程生命周期 |
| **LspClient** | `client.rs` | 与单个 Server 通信，处理请求/响应 |
| **JsonRpcConnection** | `protocol.rs` | JSON-RPC 2.0 协议实现 |
| **SymbolResolver** | `symbols.rs` | 符号名称 + 类型匹配 |
| **DiagnosticsStore** | `diagnostics.rs` | 诊断缓存与去抖 |
| **LspHandler** | `handlers/ext/lsp.rs` | Tool 请求处理 |

---

## 3. 核心组件

### 3.1 错误类型 (`error.rs`)

```rust
use thiserror::Error;

pub type Result<T> = std::result::Result<T, LspErr>;

#[derive(Error, Debug)]
pub enum LspErr {
    /// Server 二进制未找到 (需预装)
    #[error("LSP server not found: {server}. Install: {hint}")]
    ServerNotFound { server: String, hint: String },

    /// Server 启动失败
    #[error("failed to start LSP server {server}: {reason}")]
    ServerStartFailed { server: String, reason: String },

    /// 初始化超时 (45s)
    #[error("LSP server initialization timed out after {timeout_secs}s")]
    InitializationTimeout { timeout_secs: i32 },

    /// JSON-RPC 协议错误
    #[error("JSON-RPC error: {message}")]
    JsonRpc { message: String, code: Option<i32> },

    /// 无可用 Server
    #[error("no LSP server available for file extension: {ext}")]
    NoServerForExtension { ext: String },

    /// 符号未找到
    #[error("symbol '{name}' not found in {file}")]
    SymbolNotFound { name: String, file: String },

    /// 文件不存在
    #[error("file not found: {path}")]
    FileNotFound { path: String },

    /// 请求超时
    #[error("LSP request timed out after {timeout_secs}s")]
    RequestTimeout { timeout_secs: i32 },

    /// 内部错误
    #[error("internal LSP error: {0}")]
    Internal(String),

    /// IO 错误
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON 序列化错误
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
```

### 3.2 配置类型 (`config.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 内置 Server 定义
pub struct BuiltinServer {
    pub id: &'static str,
    pub extensions: &'static [&'static str],
    pub commands: &'static [&'static str],
    pub install_hint: &'static str,
}

/// 内置 Server 注册表 (仅 Rust, Go, Python)
pub const BUILTIN_SERVERS: &[BuiltinServer] = &[
    BuiltinServer {
        id: "rust-analyzer",
        extensions: &[".rs"],
        commands: &["rust-analyzer"],
        install_hint: "rustup component add rust-analyzer",
    },
    BuiltinServer {
        id: "gopls",
        extensions: &[".go"],
        commands: &["gopls"],
        install_hint: "go install golang.org/x/tools/gopls@latest",
    },
    BuiltinServer {
        id: "pyright",
        extensions: &[".py", ".pyi"],
        commands: &["pyright-langserver", "--stdio"],
        install_hint: "npm install -g pyright",
    },
];

/// 用户 LSP 配置
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct LspConfig {
    /// 全局启用 LSP
    pub enabled: bool,

    /// 每个 Server 的配置覆盖
    pub servers: HashMap<String, LspServerConfig>,
}

/// 单个 Server 配置
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(default)]
pub struct LspServerConfig {
    /// 覆盖启动命令
    pub command: Option<Vec<String>>,

    /// 额外文件扩展名
    pub extensions: Vec<String>,

    /// 禁用此 Server
    pub disabled: bool,

    /// LSP 初始化选项
    pub initialization_options: serde_json::Value,
}
```

### 3.3 符号解析 (`symbols.rs`)

```rust
use lsp_types::{DocumentSymbol, DocumentSymbolResponse, Position, SymbolKind as LspSymbolKind};
use serde::{Deserialize, Serialize};

/// 简化的符号类型 (AI 友好)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Enum,
    Variable,
    Constant,
    Property,
    Field,
    Module,
    Type,
    Other,
}

impl From<LspSymbolKind> for SymbolKind {
    fn from(kind: LspSymbolKind) -> Self {
        match kind {
            LspSymbolKind::FUNCTION => SymbolKind::Function,
            LspSymbolKind::METHOD => SymbolKind::Method,
            LspSymbolKind::CLASS => SymbolKind::Class,
            LspSymbolKind::STRUCT => SymbolKind::Struct,
            LspSymbolKind::INTERFACE => SymbolKind::Interface,
            LspSymbolKind::ENUM => SymbolKind::Enum,
            LspSymbolKind::VARIABLE => SymbolKind::Variable,
            LspSymbolKind::CONSTANT => SymbolKind::Constant,
            LspSymbolKind::PROPERTY => SymbolKind::Property,
            LspSymbolKind::FIELD => SymbolKind::Field,
            LspSymbolKind::MODULE | LspSymbolKind::NAMESPACE => SymbolKind::Module,
            LspSymbolKind::TYPE_PARAMETER => SymbolKind::Type,
            _ => SymbolKind::Other,
        }
    }
}

impl SymbolKind {
    /// 从字符串解析 (宽松匹配)
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "function" | "func" | "fn" => Some(SymbolKind::Function),
            "method" => Some(SymbolKind::Method),
            "class" => Some(SymbolKind::Class),
            "struct" => Some(SymbolKind::Struct),
            "interface" | "trait" => Some(SymbolKind::Interface),
            "enum" => Some(SymbolKind::Enum),
            "variable" | "var" | "let" => Some(SymbolKind::Variable),
            "constant" | "const" => Some(SymbolKind::Constant),
            "property" | "prop" => Some(SymbolKind::Property),
            "field" => Some(SymbolKind::Field),
            "module" | "mod" | "namespace" => Some(SymbolKind::Module),
            "type" => Some(SymbolKind::Type),
            _ => None,
        }
    }
}

/// 解析后的符号
#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub position: Position,
    pub range_start_line: i32,
    pub range_end_line: i32,
}

/// 符号匹配结果
#[derive(Debug, Clone)]
pub struct SymbolMatch {
    pub symbol: ResolvedSymbol,
    pub exact_name_match: bool,
}

/// 展平嵌套的文档符号
pub fn flatten_symbols(response: &DocumentSymbolResponse) -> Vec<ResolvedSymbol> {
    let mut result = Vec::new();

    match response {
        DocumentSymbolResponse::Flat(symbols) => {
            for sym in symbols {
                result.push(ResolvedSymbol {
                    name: sym.name.clone(),
                    kind: sym.kind.into(),
                    position: sym.location.range.start,
                    range_start_line: sym.location.range.start.line as i32,
                    range_end_line: sym.location.range.end.line as i32,
                });
            }
        }
        DocumentSymbolResponse::Nested(symbols) => {
            flatten_nested(&mut result, symbols);
        }
    }

    result
}

fn flatten_nested(result: &mut Vec<ResolvedSymbol>, symbols: &[DocumentSymbol]) {
    for sym in symbols {
        result.push(ResolvedSymbol {
            name: sym.name.clone(),
            kind: sym.kind.into(),
            position: sym.selection_range.start,
            range_start_line: sym.range.start.line as i32,
            range_end_line: sym.range.end.line as i32,
        });

        if let Some(children) = &sym.children {
            flatten_nested(result, children);
        }
    }
}

/// 查找匹配的符号
pub fn find_matching_symbols(
    symbols: &[ResolvedSymbol],
    name: &str,
    kind: Option<SymbolKind>,
) -> Vec<SymbolMatch> {
    let name_lower = name.to_lowercase();

    symbols
        .iter()
        .filter_map(|sym| {
            let sym_name_lower = sym.name.to_lowercase();
            let exact_name_match = sym_name_lower == name_lower;
            let contains_match = sym_name_lower.contains(&name_lower);

            if !exact_name_match && !contains_match {
                return None;
            }

            // 按类型过滤
            if let Some(k) = kind {
                if sym.kind != k {
                    return None;
                }
            }

            Some(SymbolMatch {
                symbol: sym.clone(),
                exact_name_match,
            })
        })
        .collect()
}
```

### 3.4 JSON-RPC 协议 (`protocol.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{timeout, Duration};

use crate::error::{LspErr, Result};

/// 默认请求超时
const REQUEST_TIMEOUT_SECS: i32 = 30;

type RequestId = i32;

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T: Serialize> {
    jsonrpc: &'static str,
    id: RequestId,
    method: String,
    params: T,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    id: Option<RequestId>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

struct PendingRequest {
    tx: oneshot::Sender<Result<serde_json::Value>>,
}

/// JSON-RPC 连接
pub struct JsonRpcConnection {
    next_id: AtomicI32,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<RequestId, PendingRequest>>>,
}

impl JsonRpcConnection {
    /// 创建连接
    pub fn new(
        stdin: ChildStdin,
        stdout: ChildStdout,
        notification_tx: mpsc::Sender<(String, serde_json::Value)>,
    ) -> Self {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = Arc::clone(&pending);

        // 启动读取任务
        tokio::spawn(async move {
            Self::read_loop(stdout, pending_clone, notification_tx).await;
        });

        Self {
            next_id: AtomicI32::new(1),
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
        }
    }

    /// 发送请求并等待响应
    pub async fn request<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, PendingRequest { tx });
        }

        // 序列化并发送
        let body = serde_json::to_string(&request)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
        }

        // 等待响应 (带超时)
        match timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS as u64), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LspErr::Internal("request cancelled".to_string())),
            Err(_) => {
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err(LspErr::RequestTimeout {
                    timeout_secs: REQUEST_TIMEOUT_SECS,
                })
            }
        }
    }

    /// 发送通知 (无响应)
    pub async fn notify<P: Serialize>(&self, method: &str, params: P) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let body = serde_json::to_string(&notification)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(message.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// 消息读取循环
    async fn read_loop(
        stdout: ChildStdout,
        pending: Arc<Mutex<HashMap<RequestId, PendingRequest>>>,
        notification_tx: mpsc::Sender<(String, serde_json::Value)>,
    ) {
        let mut reader = BufReader::new(stdout);
        let mut buffer = Vec::new();

        loop {
            // 读取 Content-Length 头
            let mut header = String::new();
            if reader.read_line(&mut header).await.is_err() {
                break;
            }

            if !header.starts_with("Content-Length:") {
                continue;
            }

            let content_length: usize = header
                .trim_start_matches("Content-Length:")
                .trim()
                .parse()
                .unwrap_or(0);

            // 跳过空行
            let mut empty = String::new();
            let _ = reader.read_line(&mut empty).await;

            // 读取消息体
            buffer.resize(content_length, 0);
            if reader.read_exact(&mut buffer).await.is_err() {
                break;
            }

            // 解析消息
            let raw = String::from_utf8_lossy(&buffer);
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if value.get("id").is_some() {
                    // 响应
                    if let Ok(response) = serde_json::from_value::<JsonRpcResponse>(value) {
                        if let Some(id) = response.id {
                            let mut pending_guard = pending.lock().await;
                            if let Some(req) = pending_guard.remove(&id) {
                                let result = if let Some(err) = response.error {
                                    Err(LspErr::JsonRpc {
                                        message: err.message,
                                        code: Some(err.code),
                                    })
                                } else {
                                    Ok(response.result.unwrap_or(serde_json::Value::Null))
                                };
                                let _ = req.tx.send(result);
                            }
                        }
                    }
                } else if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
                    // 通知
                    let params = value
                        .get("params")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let _ = notification_tx.send((method.to_string(), params)).await;
                }
            }
        }
    }
}
```

### 3.5 诊断存储 (`diagnostics.rs`)

```rust
use lsp_types::{Diagnostic, DiagnosticSeverity, PublishDiagnosticsParams};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 去抖间隔 (150ms)
const DIAGNOSTIC_DEBOUNCE_MS: u64 = 150;

/// 简化的诊断条目
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub file: PathBuf,
    pub line: i32,
    pub character: i32,
    pub severity: DiagnosticSeverityLevel,
    pub message: String,
    pub code: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticSeverityLevel {
    Error,
    Warning,
    Info,
    Hint,
}

impl From<Option<DiagnosticSeverity>> for DiagnosticSeverityLevel {
    fn from(severity: Option<DiagnosticSeverity>) -> Self {
        match severity {
            Some(DiagnosticSeverity::ERROR) => DiagnosticSeverityLevel::Error,
            Some(DiagnosticSeverity::WARNING) => DiagnosticSeverityLevel::Warning,
            Some(DiagnosticSeverity::INFORMATION) => DiagnosticSeverityLevel::Info,
            Some(DiagnosticSeverity::HINT) => DiagnosticSeverityLevel::Hint,
            None => DiagnosticSeverityLevel::Error,
            _ => DiagnosticSeverityLevel::Info,
        }
    }
}

struct FileDiagnostics {
    diagnostics: Vec<DiagnosticEntry>,
    last_update: Instant,
    version: i32,
}

/// 诊断存储 (带去抖)
pub struct DiagnosticsStore {
    files: Arc<RwLock<HashMap<PathBuf, FileDiagnostics>>>,
    dirty: Arc<RwLock<Vec<PathBuf>>>,
}

impl DiagnosticsStore {
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(HashMap::new())),
            dirty: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 更新诊断 (来自 publishDiagnostics)
    pub async fn update(&self, params: PublishDiagnosticsParams) {
        let path = PathBuf::from(params.uri.path());
        let entries: Vec<DiagnosticEntry> = params
            .diagnostics
            .into_iter()
            .map(|d| DiagnosticEntry {
                file: path.clone(),
                line: d.range.start.line as i32 + 1,
                character: d.range.start.character as i32 + 1,
                severity: d.severity.into(),
                message: d.message,
                code: d.code.map(|c| match c {
                    lsp_types::NumberOrString::Number(n) => n.to_string(),
                    lsp_types::NumberOrString::String(s) => s,
                }),
                source: d.source,
            })
            .collect();

        let now = Instant::now();

        let mut files = self.files.write().await;
        let version = files.get(&path).map(|f| f.version + 1).unwrap_or(1);

        files.insert(
            path.clone(),
            FileDiagnostics {
                diagnostics: entries,
                last_update: now,
                version,
            },
        );

        let mut dirty = self.dirty.write().await;
        if !dirty.contains(&path) {
            dirty.push(path);
        }
    }

    /// 获取文件的诊断
    pub async fn get_file(&self, path: &PathBuf) -> Vec<DiagnosticEntry> {
        let files = self.files.read().await;
        files.get(path).map(|f| f.diagnostics.clone()).unwrap_or_default()
    }

    /// 取走所有脏诊断 (用于 system_reminder)
    pub async fn take_dirty(&self) -> Vec<DiagnosticEntry> {
        let mut dirty = self.dirty.write().await;
        let files = self.files.read().await;

        let mut all_entries = Vec::new();
        for path in dirty.drain(..) {
            if let Some(file_diags) = files.get(&path) {
                if file_diags.last_update.elapsed() >= Duration::from_millis(DIAGNOSTIC_DEBOUNCE_MS) {
                    all_entries.extend(file_diags.diagnostics.clone());
                }
            }
        }

        all_entries
    }

    /// 格式化为 system_reminder
    pub fn format_for_system_reminder(entries: &[DiagnosticEntry]) -> String {
        if entries.is_empty() {
            return String::new();
        }

        let mut output = String::from("<new-diagnostics>\n");
        output.push_str("The following new diagnostic issues were detected:\n\n");

        let mut by_file: HashMap<&PathBuf, Vec<&DiagnosticEntry>> = HashMap::new();
        for entry in entries {
            by_file.entry(&entry.file).or_default().push(entry);
        }

        for (file, file_entries) in by_file {
            output.push_str(&format!("File: {}\n", file.display()));
            for entry in file_entries {
                let severity = match entry.severity {
                    DiagnosticSeverityLevel::Error => "[error]",
                    DiagnosticSeverityLevel::Warning => "[warning]",
                    DiagnosticSeverityLevel::Info => "[info]",
                    DiagnosticSeverityLevel::Hint => "[hint]",
                };
                let code_str = entry.code.as_ref().map(|c| format!(" [{}]", c)).unwrap_or_default();
                let source_str = entry.source.as_ref().map(|s| format!(" ({})", s)).unwrap_or_default();
                output.push_str(&format!(
                    "Line {}: {} {}{}{}\n",
                    entry.line, severity, entry.message, code_str, source_str
                ));
            }
            output.push('\n');
        }

        output.push_str("</new-diagnostics>");
        output
    }
}

impl Default for DiagnosticsStore {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## 4. LSP Tool 规范

### 4.1 Tool 定义

| 属性 | 值 |
|------|------|
| **名称** | `lsp` |
| **并行安全** | `true` (可并行执行) |
| **只读** | `true` (不修改文件) |
| **Feature Flag** | `Feature::Lsp` |

### 4.2 输入 Schema

```typescript
interface LspToolInput {
  /**
   * LSP 操作
   */
  operation: "goToDefinition" | "findReferences" | "hover" | "documentSymbol" | "getDiagnostics";

  /**
   * 文件路径 (绝对或相对)
   */
  filePath: string;

  /**
   * 符号名称 (除 documentSymbol/getDiagnostics 外必需)
   */
  symbolName?: string;

  /**
   * 符号类型过滤 (可选)
   */
  symbolKind?: "function" | "method" | "class" | "struct" | "interface" |
               "enum" | "variable" | "constant" | "property" | "field" |
               "module" | "type";
}
```

### 4.3 输出 Schema

```typescript
interface LspToolOutput {
  /**
   * 执行的操作
   */
  operation: string;

  /**
   * 格式化的结果
   */
  result: string;

  /**
   * 操作是否成功
   */
  success: boolean;
}
```

### 4.4 操作说明

| 操作 | LSP 方法 | 必需参数 | 描述 |
|------|----------|---------|------|
| `goToDefinition` | `textDocument/definition` | symbolName | 跳转到符号定义 |
| `findReferences` | `textDocument/references` | symbolName | 查找所有引用 |
| `hover` | `textDocument/hover` | symbolName | 获取类型/文档信息 |
| `documentSymbol` | `textDocument/documentSymbol` | - | 列出文件所有符号 |
| `getDiagnostics` | 本地缓存 | - | 获取文件诊断 |

### 4.5 使用示例

**Go to Definition:**
```json
{
  "operation": "goToDefinition",
  "filePath": "src/main.rs",
  "symbolName": "Config",
  "symbolKind": "struct"
}
```

**输出:**
```
Found 1 definition(s) for 'Config':
  /project/src/config.rs:15:1
```

**Find References:**
```json
{
  "operation": "findReferences",
  "filePath": "src/lib.rs",
  "symbolName": "process_data",
  "symbolKind": "function"
}
```

**输出:**
```
Found 5 reference(s) for 'process_data' in 3 file(s):

/project/src/lib.rs:
  Line 45
  Line 78

/project/src/main.rs:
  Line 23

/project/tests/test_lib.rs:
  Line 12
  Line 34
```

---

## 5. 配置系统

### 5.1 Feature Flag

```rust
// core/src/features.rs
pub enum Feature {
    // ... 其他 features
    Lsp,
}

// core/src/features_ext.rs
pub(crate) const EXT_FEATURES: &[FeatureSpec] = &[
    // ... 其他 features
    FeatureSpec {
        id: Feature::Lsp,
        key: "lsp",
        stage: Stage::Experimental,
        default_enabled: false,
    },
];
```

### 5.2 配置 Schema

```rust
// protocol/src/config_types_ext.rs
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct LspConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub servers: HashMap<String, LspServerConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct LspServerConfig {
    pub command: Option<Vec<String>>,
    pub extensions: Vec<String>,
    pub disabled: bool,
    pub initialization_options: serde_json::Value,
}
```

### 5.3 配置示例

```toml
# ~/.codex/config.toml

[features]
lsp = true

[lsp]
enabled = true

# 覆盖 rust-analyzer 配置
[lsp.servers.rust-analyzer]
initialization_options = { checkOnSave = { command = "clippy" } }

# 禁用 pyright
[lsp.servers.pyright]
disabled = true

# 自定义 Go LSP
[lsp.servers.gopls]
command = ["/custom/path/gopls", "-remote=auto"]
```

---

## 6. 文件同步机制

### 6.1 同步策略

采用 **On-demand sync + didChangeWatchedFiles** 策略：

```
┌─────────────────────────────────────────────────────────────┐
│                    File Sync Strategy                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. LSP Tool 调用时                                          │
│     └─► 检查文件是否已 open                                  │
│         ├─► 未 open: 发送 textDocument/didOpen               │
│         └─► 已 open: 检查 mtime/hash → 发送 didChange        │
│                                                              │
│  2. Write/Edit Tool 完成后                                   │
│     └─► 发送 workspace/didChangeWatchedFiles                │
│         通知 LSP Server 文件已更新                           │
│                                                              │
│  3. LSP Server 索引                                          │
│     └─► Server 自动扫描 workspace 文件                       │
│     └─► 重新索引变更文件 (后台异步)                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 文件追踪

```rust
/// 已打开文件状态
struct OpenedFile {
    version: i32,
    content_hash: u64,
}

/// 文件追踪器
pub struct FileTracker {
    opened: HashMap<PathBuf, OpenedFile>,
}

impl FileTracker {
    /// 同步文件到 LSP Server
    pub async fn sync_file(&mut self, path: &Path, connection: &JsonRpcConnection) -> Result<Url> {
        let uri = Url::from_file_path(path)?;
        let content = tokio::fs::read_to_string(path).await?;
        let content_hash = calculate_hash(&content);

        if let Some(file) = self.opened.get_mut(&path.to_path_buf()) {
            // 已打开 - 检查是否变化
            if file.content_hash != content_hash {
                file.version += 1;
                file.content_hash = content_hash;

                connection.notify("textDocument/didChange", json!({
                    "textDocument": { "uri": uri.to_string(), "version": file.version },
                    "contentChanges": [{ "text": content }]
                })).await?;
            }
        } else {
            // 首次打开
            let lang_id = detect_language(path);
            connection.notify("textDocument/didOpen", json!({
                "textDocument": {
                    "uri": uri.to_string(),
                    "languageId": lang_id,
                    "version": 1,
                    "text": content
                }
            })).await?;

            self.opened.insert(path.to_path_buf(), OpenedFile {
                version: 1,
                content_hash,
            });
        }

        Ok(uri)
    }

    /// 通知文件变更 (来自 Write/Edit tool)
    pub async fn notify_file_changed(&self, paths: &[PathBuf], connection: &JsonRpcConnection) -> Result<()> {
        let changes: Vec<_> = paths.iter().map(|p| {
            json!({
                "uri": Url::from_file_path(p).unwrap().to_string(),
                "type": 2  // Changed
            })
        }).collect();

        connection.notify("workspace/didChangeWatchedFiles", json!({
            "changes": changes
        })).await
    }
}
```

### 6.3 关于跨文件引用

**问题**: 如果文件 A 被修改，文件 B 新增了对 A 的引用，查询 "谁使用了 A" 时会丢失 B 吗？

**答案**: 不会！

```
LSP Server 索引机制:
┌──────────────────────────────────────────────────────────────┐
│                                                               │
│   LSP Server 维护 Workspace 索引                              │
│   ┌─────────────────────────────────────────────────────────┐│
│   │ rust-analyzer / gopls / pyright                         ││
│   │                                                         ││
│   │ - 后台扫描整个 workspace 的所有文件                      ││
│   │ - 构建 AST/Symbol 索引                                  ││
│   │ - 文件来源: 磁盘文件 (不仅是 didOpen 的文件)            ││
│   │                                                         ││
│   │ didOpen/didChange 用于:                                 ││
│   │ - 未保存的修改 (dirty buffer)                           ││
│   │ - 实时诊断反馈                                          ││
│   └─────────────────────────────────────────────────────────┘│
│                                                               │
│   CLI 模式下文件已写入磁盘 → Server 会重新索引 → 能找到 B    │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

---

## 7. 符号解析策略

### 7.1 为什么使用符号名称

传统 LSP 使用精确的行/列位置，但这对 LLM 不友好：
- LLM 容易产生错误的行列号
- 代码编辑后位置失效
- 需要先读取文件确定位置

Codex LSP 使用 **符号名称 + 类型** 解析：
- 更符合人类描述方式 ("找到 Config 结构体的定义")
- 自动处理代码变更
- 支持模糊匹配

### 7.2 解析流程

```
用户请求: findReferences("process_data", kind="function")
    │
    ▼
1. sync_file(path) - 同步文件到 LSP
    │
    ▼
2. textDocument/documentSymbol - 获取所有符号
    │
    │  响应:
    │  ┌─────────────────────────────────┐
    │  │ DocumentSymbol (struct Config)  │
    │  │ DocumentSymbol (fn main)        │
    │  │ DocumentSymbol (fn process_data)│ ← 匹配!
    │  │   └─ position: {line: 45, char: 0} │
    │  │ DocumentSymbol (const VERSION)  │
    │  └─────────────────────────────────┘
    │
    ▼
3. flatten_symbols() - 展平嵌套符号
    │
    ▼
4. find_matching_symbols(name, kind) - 匹配
    │
    │  匹配结果:
    │  ┌─────────────────────────────────┐
    │  │ process_data (function)         │
    │  │ position: {line: 45, char: 0}   │
    │  │ exact_match: true               │
    │  └─────────────────────────────────┘
    │
    ▼
5. textDocument/references - 使用解析出的位置
    │
    ▼
6. 返回格式化结果
```

### 7.3 匹配规则

```rust
/// 符号匹配规则
fn match_symbol(sym: &ResolvedSymbol, name: &str, kind: Option<SymbolKind>) -> Option<SymbolMatch> {
    let name_lower = name.to_lowercase();
    let sym_name_lower = sym.name.to_lowercase();

    // 1. 精确匹配
    let exact_match = sym_name_lower == name_lower;

    // 2. 包含匹配 (fallback)
    let contains_match = sym_name_lower.contains(&name_lower);

    if !exact_match && !contains_match {
        return None;
    }

    // 3. 类型过滤 (如果指定)
    if let Some(k) = kind {
        if sym.kind != k {
            return None;
        }
    }

    Some(SymbolMatch {
        symbol: sym.clone(),
        exact_name_match: exact_match,
    })
}

// 优先返回精确匹配的结果
matches.sort_by(|a, b| b.exact_name_match.cmp(&a.exact_name_match));
```

---

## 8. 诊断系统集成

### 8.1 诊断流程

```
┌─────────────────────────────────────────────────────────────────┐
│                    Diagnostics Integration                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. 文件变更                                                     │
│     └─► Claude 通过 Write/Edit tool 修改文件                    │
│     └─► sync_file() 通知 LSP Server                             │
│                                                                  │
│  2. LSP 分析                                                     │
│     └─► Server 分析文件                                         │
│     └─► 生成诊断                                                │
│                                                                  │
│  3. 推送通知                                                     │
│     └─► textDocument/publishDiagnostics                         │
│     └─► DiagnosticsStore.update()                               │
│     └─► 标记为 dirty                                            │
│                                                                  │
│  4. 去抖 (150ms)                                                 │
│     └─► 等待语义分析完成                                        │
│     └─► 合并多次更新                                            │
│                                                                  │
│  5. 生成 Attachment                                              │
│     └─► 对话 turn 结束时                                        │
│     └─► take_dirty() 获取新诊断                                 │
│     └─► format_for_system_reminder()                            │
│                                                                  │
│  6. 注入对话                                                     │
│     └─► <new-diagnostics>...</new-diagnostics>                  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2 System Reminder 格式

```xml
<new-diagnostics>
The following new diagnostic issues were detected:

File: /project/src/main.rs
Line 15: [error] cannot find value `undeclared_var` in this scope [E0425] (rust-analyzer)
Line 23: [warning] unused variable: `temp` [unused_variables] (rust-analyzer)

File: /project/src/lib.rs
Line 45: [error] mismatched types: expected `i32`, found `String` [E0308] (rust-analyzer)

</new-diagnostics>
```

### 8.3 集成点

```rust
// 在对话处理循环中
async fn generate_system_attachments(&self) -> Vec<Attachment> {
    let mut attachments = Vec::new();

    // ... 其他 attachments ...

    // LSP 诊断
    if let Some(manager) = self.lsp_manager.as_ref() {
        let dirty = manager.diagnostics().take_dirty().await;
        if !dirty.is_empty() {
            let formatted = DiagnosticsStore::format_for_system_reminder(&dirty);
            attachments.push(Attachment::SystemReminder {
                content: formatted,
            });
        }
    }

    attachments
}
```

---

## 9. 服务器生命周期管理

### 9.1 Server Manager

```rust
pub struct LspServerManager {
    /// 活跃连接: key = "{server_id}:{root}"
    servers: RwLock<HashMap<String, ServerState>>,
    /// 启动失败的 Server
    broken: RwLock<HashSet<String>>,
    /// 用户配置
    config: LspConfig,
    /// 共享诊断存储
    diagnostics: Arc<DiagnosticsStore>,
}

struct ServerState {
    client: Arc<LspClient>,
    server_id: String,
    root: PathBuf,
}
```

### 9.2 生命周期

```
┌─────────────────────────────────────────────────────────────────┐
│                    Server Lifecycle                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. 首次请求                                                     │
│     └─► get_client(file_path)                                   │
│     └─► 查找匹配的 BuiltinServer (按扩展名)                      │
│     └─► 检查是否在 broken 列表                                   │
│     └─► 检查是否已有连接 (复用)                                  │
│                                                                  │
│  2. 启动 Server                                                  │
│     └─► which::which(binary) - 检查是否安装                      │
│     └─► 未安装: 返回 ServerNotFound + install_hint              │
│     └─► 已安装: spawn 进程                                       │
│                                                                  │
│  3. 初始化                                                       │
│     └─► 发送 initialize 请求 (45s 超时)                          │
│     └─► 发送 initialized 通知                                    │
│     └─► 启动 notification 处理循环                               │
│                                                                  │
│  4. 运行期                                                       │
│     └─► 处理 tool 请求                                          │
│     └─► 接收 publishDiagnostics                                 │
│     └─► 连接复用 (同 server_id + root)                           │
│                                                                  │
│  5. 关闭                                                         │
│     └─► 发送 shutdown 请求                                       │
│     └─► 发送 exit 通知                                          │
│     └─► 终止进程                                                │
│                                                                  │
│  错误恢复:                                                       │
│     └─► 启动失败: 标记为 broken, 下次跳过                        │
│     └─► 运行时错误: 返回错误, 不自动重启                         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 9.3 项目根目录检测

```rust
async fn find_root(&self, file: &Path, server: &BuiltinServer) -> PathBuf {
    let markers = match server.id {
        "rust-analyzer" => vec!["Cargo.toml"],
        "gopls" => vec!["go.mod", "go.sum", "go.work"],
        "pyright" => vec!["pyproject.toml", "setup.py", "requirements.txt"],
        _ => vec![],
    };

    let mut dir = file.parent();
    while let Some(d) = dir {
        for marker in &markers {
            if d.join(marker).exists() {
                return d.to_path_buf();
            }
        }
        dir = d.parent();
    }

    // 默认: 文件所在目录
    file.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
}
```

---

## 10. 错误处理

### 10.1 错误转换

```rust
// LSP crate 内部错误
pub enum LspErr {
    ServerNotFound { server, hint },
    ServerStartFailed { server, reason },
    InitializationTimeout { timeout_secs },
    JsonRpc { message, code },
    NoServerForExtension { ext },
    SymbolNotFound { name, file },
    FileNotFound { path },
    RequestTimeout { timeout_secs },
    Internal(String),
    // ...
}

// Handler 中转换为 FunctionCallError
impl LspHandler {
    fn convert_error(e: LspErr) -> FunctionCallError {
        match e {
            LspErr::ServerNotFound { server, hint } => {
                FunctionCallError::RespondToModel(format!(
                    "LSP server '{}' not found. Install: {}",
                    server, hint
                ))
            }
            LspErr::SymbolNotFound { name, file } => {
                FunctionCallError::RespondToModel(format!(
                    "Symbol '{}' not found in {}",
                    name, file
                ))
            }
            LspErr::NoServerForExtension { ext } => {
                FunctionCallError::RespondToModel(format!(
                    "No LSP server available for {} files. Supported: .rs, .go, .py",
                    ext
                ))
            }
            _ => FunctionCallError::RespondToModel(e.to_string()),
        }
    }
}
```

### 10.2 用户友好消息

| 错误类型 | 用户消息 |
|----------|---------|
| Server 未安装 | `LSP server 'rust-analyzer' not found. Install: rustup component add rust-analyzer` |
| 符号未找到 | `Symbol 'Config' not found in src/main.rs` |
| 不支持的文件 | `No LSP server available for .txt files. Supported: .rs, .go, .py` |
| 请求超时 | `LSP request timed out after 30s` |

---

## 11. 测试策略

### 11.1 单元测试

```rust
// lsp/src/symbols.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_from_str() {
        assert_eq!(SymbolKind::from_str_loose("function"), Some(SymbolKind::Function));
        assert_eq!(SymbolKind::from_str_loose("fn"), Some(SymbolKind::Function));
        assert_eq!(SymbolKind::from_str_loose("STRUCT"), Some(SymbolKind::Struct));
        assert_eq!(SymbolKind::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_find_matching_symbols() {
        let symbols = vec![
            ResolvedSymbol {
                name: "process_data".to_string(),
                kind: SymbolKind::Function,
                position: Position { line: 10, character: 0 },
                range_start_line: 10,
                range_end_line: 20,
            },
            ResolvedSymbol {
                name: "ProcessData".to_string(),
                kind: SymbolKind::Struct,
                position: Position { line: 5, character: 0 },
                range_start_line: 5,
                range_end_line: 8,
            },
        ];

        // 精确匹配
        let matches = find_matching_symbols(&symbols, "process_data", Some(SymbolKind::Function));
        assert_eq!(matches.len(), 1);
        assert!(matches[0].exact_name_match);

        // 不区分大小写
        let matches = find_matching_symbols(&symbols, "PROCESS_DATA", None);
        assert_eq!(matches.len(), 2);

        // 类型过滤
        let matches = find_matching_symbols(&symbols, "process", Some(SymbolKind::Struct));
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].symbol.kind, SymbolKind::Struct);
    }
}
```

### 11.2 集成测试

```rust
// lsp/tests/integration.rs
#[tokio::test]
async fn test_rust_analyzer_integration() {
    // 跳过如果未安装
    if which::which("rust-analyzer").is_err() {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    // 创建项目
    std::fs::write(root.join("Cargo.toml"), r#"
[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#).unwrap();

    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), r#"
pub struct Config {
    pub name: String,
}

pub fn process(config: &Config) -> String {
    config.name.clone()
}
"#).unwrap();

    let diagnostics = Arc::new(DiagnosticsStore::new());
    let config = LspConfig::default();
    let manager = LspServerManager::new(config, diagnostics);

    let client = manager.get_client(&root.join("src/lib.rs")).await.unwrap();

    // 测试 document symbols
    let symbols = client.document_symbols(&root.join("src/lib.rs")).await.unwrap();
    assert!(symbols.iter().any(|s| s.name == "Config"));
    assert!(symbols.iter().any(|s| s.name == "process"));

    // 测试 definition
    let locs = client.definition(&root.join("src/lib.rs"), "Config", Some(SymbolKind::Struct)).await.unwrap();
    assert!(!locs.is_empty());

    manager.shutdown().await;
}
```

### 11.3 Handler 测试

```rust
// core/src/tools/handlers/ext/lsp.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_args_parsing() {
        let json = r#"{
            "operation": "goToDefinition",
            "filePath": "src/lib.rs",
            "symbolName": "MyStruct",
            "symbolKind": "struct"
        }"#;

        let args: LspArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.operation, "goToDefinition");
        assert_eq!(args.file_path, "src/lib.rs");
        assert_eq!(args.symbol_name, Some("MyStruct".to_string()));
        assert_eq!(args.symbol_kind, Some("struct".to_string()));
    }

    #[test]
    fn test_lsp_args_minimal() {
        let json = r#"{
            "operation": "documentSymbol",
            "filePath": "src/lib.rs"
        }"#;

        let args: LspArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.operation, "documentSymbol");
        assert!(args.symbol_name.is_none());
    }
}
```

---

## 12. 实现路径

### Phase 1: LSP Crate 基础 (复杂度: 中)

**文件:**
- `codex-rs/lsp/Cargo.toml`
- `codex-rs/lsp/src/lib.rs`
- `codex-rs/lsp/src/error.rs`
- `codex-rs/lsp/src/config.rs`
- `codex-rs/lsp/src/symbols.rs`

**任务:**
1. 创建 crate，添加依赖
2. 定义 LspErr 错误类型
3. 定义 BuiltinServer + BUILTIN_SERVERS
4. 实现符号解析 (flatten + match)

### Phase 2: 协议与客户端 (复杂度: 高)

**文件:**
- `codex-rs/lsp/src/protocol.rs`
- `codex-rs/lsp/src/client.rs`
- `codex-rs/lsp/src/diagnostics.rs`

**任务:**
1. JSON-RPC 2.0 + Content-Length framing
2. 请求/响应关联 (30s 超时)
3. 通知处理 (publishDiagnostics)
4. LspClient: sync_file, document_symbols, definition, references, hover
5. DiagnosticsStore (150ms 去抖)

### Phase 3: Server Manager (复杂度: 中)

**文件:**
- `codex-rs/lsp/src/server.rs`

**任务:**
1. 二进制检测 (which)
2. 懒启动 (首次请求时)
3. 连接复用 (server_id + root)
4. 优雅关闭

### Phase 4: Core 集成 (复杂度: 低)

**文件:**
- `codex-rs/core/src/tools/ext/lsp.rs` (NEW)
- `codex-rs/core/src/tools/handlers/ext/lsp.rs` (NEW)
- `codex-rs/core/src/tools/spec_ext.rs` (MODIFY - ~5 行)
- `codex-rs/core/src/features.rs` (MODIFY - 1 行)
- `codex-rs/core/src/features_ext.rs` (MODIFY - ~6 行)
- `codex-rs/protocol/src/config_types_ext.rs` (MODIFY - ~30 行)

**任务:**
1. 创建 tool spec (operations: goToDefinition, findReferences, hover, documentSymbol, getDiagnostics)
2. 实现 handler
3. 注册到 spec_ext.rs
4. 添加 Feature::Lsp
5. 添加 LspConfig 到 protocol types

### Phase 5: 测试 (复杂度: 中)

**任务:**
1. 单元测试: 符号解析, JSON-RPC framing, config 解析
2. 集成测试: rust-analyzer smoke test (跳过如果未安装)
3. Handler 测试: 参数解析, 错误处理

---

## 13. 参考实现对比

| 特性 | OpenCode | Claude Code | cclsp | **Codex (设计)** |
|------|----------|-------------|-------|-----------------|
| **语言支持** | 40+ | 插件扩展 | 15+ | 3 (Rust/Go/Python) |
| **Server 安装** | 自动下载 | 需预装 | 需预装 | 需预装 |
| **符号定位** | 行/列 | 行/列 | 名称+类型 | 名称+类型 |
| **文件同步** | On-demand | On-demand | On-demand | On-demand + didChangeWatchedFiles |
| **诊断** | 自动推送 | 自动推送 | On-demand | 自动推送 (system_reminder) |
| **Feature Flag** | 实验性 | 环境变量 | 始终启用 | Feature::Lsp |
| **配置** | opencode.json | .lsp.json | cclsp.json | config.toml [lsp] |

### 关键设计借鉴

| 来源 | 借鉴内容 |
|------|---------|
| **OpenCode** | 45s 初始化超时, 150ms 诊断去抖, Server lifecycle |
| **cclsp** | 符号名称+类型解析, 文件版本追踪 |
| **Claude Code** | system_reminder 诊断格式, Feature flag 模式 |

---

## 附录 A: 依赖项

```toml
# codex-rs/lsp/Cargo.toml
[dependencies]
tokio = { workspace = true, features = ["process", "io-util", "sync", "time"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
lsp-types = "0.95"
thiserror = { workspace = true }
tracing = { workspace = true }
which = "7.0"
codex-file-ignore = { path = "../file-ignore" }

[dev-dependencies]
tempfile = { workspace = true }
tokio-test = { workspace = true }
pretty_assertions = { workspace = true }
```

## 附录 B: LSP 服务器安装

```bash
# Rust
rustup component add rust-analyzer

# Go
go install golang.org/x/tools/gopls@latest

# Python
npm install -g pyright
# 或
pip install pyright
```

## 附录 C: 调试

```bash
# 启用 LSP 日志
RUST_LOG=codex_lsp=debug codex

# 检查 Server 是否可用
which rust-analyzer
which gopls
which pyright-langserver
```

---

*文档生成日期: 2025-12-28*
*基于 OpenCode, Claude Code, cclsp 最佳实践设计*
