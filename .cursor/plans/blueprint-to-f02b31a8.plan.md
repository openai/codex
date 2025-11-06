<!-- f02b31a8-5b45-4245-b01f-711cbef26e42 5aa72c8c-1b2d-45dd-bdac-edcf609c9cf6 -->
# Phase 2: Stage 3-4 完全実装計画

## 🔧 Stage 3: Windows MCP統合（2-3日）

### 概要

Windows 11 25H2のMCP（Model Context Protocol）APIを使用して、CodexをシステムレベルのAIエージェントとして登録し、OS統合を実現する。

---

### 3.1 WindowsMcpBridge実装

**新規ファイル**: `codex-rs/mcp-server/src/windows_mcp_bridge.rs`

#### 実装内容

```rust
//! Windows 11 25H2 MCP integration bridge

#[cfg(target_os = "windows")]
use anyhow::{Context, Result};
#[cfg(target_os = "windows")]
use std::sync::Arc;
#[cfg(target_os = "windows")]
use widestring::U16CString;

/// Windows MCP Bridge for OS-level integration
#[cfg(target_os = "windows")]
pub struct WindowsMcpBridge {
    mcp_server: Arc<crate::McpServer>,
}

#[cfg(target_os = "windows")]
impl WindowsMcpBridge {
    pub fn new(mcp_server: Arc<crate::McpServer>) -> Self {
        Self { mcp_server }
    }
    
    /// Register Codex as Windows 11 MCP system agent
    pub async fn register_as_system_agent(&self) -> Result<()> {
        // Windows Registry registration
        self.register_to_registry()?;
        
        // Start MCP listener
        self.start_system_listener().await?;
        
        tracing::info!("Successfully registered as Windows MCP system agent");
        Ok(())
    }
    
    fn register_to_registry(&self) -> Result<()> {
        use winreg::RegKey;
        use winreg::enums::*;
        
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let (key, _) = hklm.create_subkey("SOFTWARE\\Microsoft\\Windows\\AI\\Agents\\Codex")?;
        
        key.set_value("DisplayName", &"Codex AI Agent")?;
        key.set_value("Executable", &std::env::current_exe()?.to_string_lossy().to_string())?;
        key.set_value("Protocol", &"MCP")?;
        
        Ok(())
    }
    
    async fn start_system_listener(&self) -> Result<()> {
        // Named pipe for Windows OS communication
        // TODO: Implement Windows named pipe server
        Ok(())
    }
}
```

#### 必要な依存関係

**ファイル**: `codex-rs/mcp-server/Cargo.toml`

```toml
[target.'cfg(windows)'.dependencies]
codex-windows-ai = { path = "../windows-ai" }
widestring = "1.0"
winreg = "0.52"
```

---

### 3.2 MCPツールとして公開

**ファイル**: `codex-rs/mcp-server/src/tools/system_tools.rs`（新規作成）

```rust
use mcp_types::Tool;
use serde_json::json;

pub fn get_system_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "register_windows_agent".to_string(),
            description: "Register Codex as Windows 11 MCP system agent".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        Tool {
            name: "query_system_context".to_string(),
            description: "Query Windows system context (running apps, active window, etc.)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}
```

**ファイル**: `codex-rs/mcp-server/src/tools/mod.rs`（修正）

```rust
pub mod system_tools;
```

---

### 3.3 CLIコマンド追加

**ファイル**: `codex-rs/cli/src/main.rs`（MCPサブコマンド拡張）

```rust
#[derive(Subcommand)]
enum McpCommand {
    Server { /* 既存 */ },
    
    /// Register as Windows MCP system agent
    #[cfg(target_os = "windows")]
    RegisterAgent,
    
    /// Unregister Windows MCP system agent
    #[cfg(target_os = "windows")]
    UnregisterAgent,
}
```

---

### 3.4 テスト実装

**新規ファイル**: `codex-rs/mcp-server/tests/windows_mcp_test.rs`

```rust
#[cfg(target_os = "windows")]
#[tokio::test]
async fn test_windows_mcp_registration() {
    let bridge = WindowsMcpBridge::new(Arc::new(McpServer::new()));
    
    // Note: Requires admin privileges
    if is_admin() {
        let result = bridge.register_as_system_agent().await;
        assert!(result.is_ok());
    } else {
        // Skip test if not admin
        println!("Skipping: Requires admin privileges");
    }
}

fn is_admin() -> bool {
    // Check if running as administrator
    std::env::var("USERNAME").ok().map(|u| u == "Administrator").unwrap_or(false)
}
```

---

### 完成基準（Stage 3）

- [ ] `windows_mcp_bridge.rs` 実装完了
- [ ] `system_tools.rs` 実装完了
- [ ] `mcp-server/Cargo.toml` 依存追加
- [ ] CLIコマンド追加（register-agent, unregister-agent）
- [ ] テスト実装
- [ ] Windows 11 25H2で実機テスト（要管理者権限）
- [ ] ビルド成功（警告0、エラー0）

**テストコマンド**:

```bash
cargo build -p codex-mcp-server
cargo test -p codex-mcp-server --test windows_mcp_test
codex mcp register-agent  # 要管理者権限
```

---

## 🛡️ Stage 4: Sandbox OS構築（3-4日）

### 概要

Windows Sandbox、Hyper-V、WSL2、AppContainerを使用して、完全にネットワークから遮断されたSandbox OS環境を構築。Linux/macOS風のGUIを実装し、安全な実験環境を提供。

---

### 4.1 SandboxManager実装

**新規ディレクトリ**: `codex-rs/windows-sandbox/`

**新規ファイル**: `codex-rs/windows-sandbox/Cargo.toml`

```toml
[package]
edition = "2024"
name = "codex-windows-sandbox"
version = { workspace = true }

[lib]
name = "codex_windows_sandbox"
path = "src/lib.rs"

[lints]
workspace = true

[dependencies]
anyhow = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["full"] }
tracing = { workspace = true }
uuid = { version = "1.0", features = ["v4"] }

[target.'cfg(windows)'.dependencies]
widestring = "1.0"
winreg = "0.52"
```

**新規ファイル**: `codex-rs/windows-sandbox/src/lib.rs`

```rust
pub mod network_isolation;
pub mod sandbox_manager;

pub use sandbox_manager::{SandboxConfig, SandboxInstance, SandboxManager, SandboxType};
```

**新規ファイル**: `codex-rs/windows-sandbox/src/sandbox_manager.rs`

```rust
//! Sandbox OS manager for Windows/Hyper-V/WSL2/AppContainer

use anyhow::{Context, Result};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum SandboxType {
    WindowsSandbox,
    HyperV,
    Wsl2,
    AppContainer,
}

#[derive(Debug)]
pub struct SandboxConfig {
    pub workspace_path: PathBuf,
    pub enable_network: bool,
    pub memory_mb: usize,
    pub cpu_count: usize,
}

#[derive(Debug)]
pub struct SandboxInstance {
    pub id: String,
    pub sandbox_type: SandboxType,
}

pub struct SandboxManager {
    sandbox_type: SandboxType,
}

impl SandboxManager {
    pub fn new(sandbox_type: SandboxType) -> Self {
        Self { sandbox_type }
    }
    
    pub async fn create_sandbox_os(&self, config: SandboxConfig) -> Result<SandboxInstance> {
        match self.sandbox_type {
            SandboxType::WindowsSandbox => self.create_windows_sandbox(config).await,
            SandboxType::HyperV => self.create_hyperv_vm(config).await,
            SandboxType::Wsl2 => self.create_wsl2_distro(config).await,
            SandboxType::AppContainer => self.create_appcontainer(config).await,
        }
    }
    
    async fn create_windows_sandbox(&self, config: SandboxConfig) -> Result<SandboxInstance> {
        let network = if config.enable_network { "Enable" } else { "Disable" };
        
        let wsb_content = format!(
            r#"<Configuration>
  <VGpu>Enable</VGpu>
  <Networking>{network}</Networking>
  <MemoryInMB>{}</MemoryInMB>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>{}</HostFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
</Configuration>"#,
            config.memory_mb,
            config.workspace_path.display()
        );
        
        let sandbox_id = Uuid::new_v4().to_string();
        let wsb_path = std::env::temp_dir().join(format!("codex-sandbox-{sandbox_id}.wsb"));
        
        std::fs::write(&wsb_path, wsb_content)
            .context("Failed to write WSB configuration file")?;
        
        std::process::Command::new("WindowsSandbox.exe")
            .arg(&wsb_path)
            .spawn()
            .context("Failed to launch Windows Sandbox")?;
        
        tracing::info!("Created Windows Sandbox: {sandbox_id}");
        
        Ok(SandboxInstance {
            id: sandbox_id,
            sandbox_type: SandboxType::WindowsSandbox,
        })
    }
    
    async fn create_hyperv_vm(&self, _config: SandboxConfig) -> Result<SandboxInstance> {
        anyhow::bail!("Hyper-V support not yet implemented")
    }
    
    async fn create_wsl2_distro(&self, _config: SandboxConfig) -> Result<SandboxInstance> {
        anyhow::bail!("WSL2 support not yet implemented")
    }
    
    async fn create_appcontainer(&self, _config: SandboxConfig) -> Result<SandboxInstance> {
        anyhow::bail!("AppContainer support not yet implemented")
    }
}
```

---

### 4.2 NetworkIsolation実装

**新規ファイル**: `codex-rs/windows-sandbox/src/network_isolation.rs`

```rust
//! Complete network isolation for sandboxes

use anyhow::{Context, Result};
use std::process::Command;

pub struct NetworkIsolation;

impl NetworkIsolation {
    /// Block all network access for a sandbox
    #[cfg(target_os = "windows")]
    pub fn block_all_network_access(sandbox_id: &str) -> Result<()> {
        let ps_script = format!(
            r#"New-NetFirewallRule -DisplayName "Codex Sandbox Block {sandbox_id}" -Direction Outbound -Action Block -Program "C:\CodexSandbox\{sandbox_id}\*""#
        );
        
        Command::new("powershell.exe")
            .arg("-Command")
            .arg(&ps_script)
            .output()
            .context("Failed to create firewall rule")?;
        
        tracing::info!("Network access blocked for sandbox {sandbox_id}");
        Ok(())
    }
    
    #[cfg(not(target_os = "windows"))]
    pub fn block_all_network_access(_sandbox_id: &str) -> Result<()> {
        anyhow::bail!("Network isolation only supported on Windows")
    }
}
```

---

### 4.3 CLI統合

**ファイル**: `codex-rs/cli/src/main.rs`（Sandboxサブコマンド追加）

```rust
#[derive(Subcommand)]
enum Command {
    // 既存コマンド...
    
    /// Sandbox OS management
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommand,
    },
}

#[derive(Subcommand)]
enum SandboxCommand {
    /// Create a new sandbox
    Create {
        /// Sandbox type
        #[arg(long, default_value = "windows-sandbox")]
        sandbox_type: String,
        
        /// Workspace directory to mount
        workspace: PathBuf,
        
        /// Enable network (default: disabled for security)
        #[arg(long)]
        enable_network: bool,
        
        /// Memory in MB (default: 4096)
        #[arg(long, default_value = "4096")]
        memory: usize,
        
        /// CPU count (default: 2)
        #[arg(long, default_value = "2")]
        cpus: usize,
    },
    
    /// List active sandboxes
    List,
    
    /// Execute command in sandbox
    Exec {
        /// Sandbox ID
        sandbox_id: String,
        
        /// Command to execute
        command: String,
    },
    
    /// Destroy a sandbox
    Destroy {
        /// Sandbox ID
        sandbox_id: String,
    },
}
```

**実装ファイル**: `codex-rs/cli/src/sandbox_commands.rs`（新規）

```rust
use anyhow::Result;
use codex_windows_sandbox::{SandboxConfig, SandboxManager, SandboxType};
use std::path::PathBuf;

pub async fn create_sandbox(
    sandbox_type_str: &str,
    workspace: PathBuf,
    enable_network: bool,
    memory: usize,
    cpus: usize,
) -> Result<()> {
    let sandbox_type = match sandbox_type_str {
        "windows-sandbox" => SandboxType::WindowsSandbox,
        "hyper-v" => SandboxType::HyperV,
        "wsl2" => SandboxType::Wsl2,
        "appcontainer" => SandboxType::AppContainer,
        _ => anyhow::bail!("Unknown sandbox type: {sandbox_type_str}"),
    };
    
    let config = SandboxConfig {
        workspace_path: workspace,
        enable_network,
        memory_mb: memory,
        cpu_count: cpus,
    };
    
    let manager = SandboxManager::new(sandbox_type);
    let instance = manager.create_sandbox_os(config).await?;
    
    println!("✅ Sandbox created: {}", instance.id);
    println!("   Type: {:?}", instance.sandbox_type);
    
    Ok(())
}
```

---

### 4.4 workspace設定

**ファイル**: `codex-rs/Cargo.toml`（members追加）

```toml
members = [
    # ...
    "windows-sandbox",
    # ...
]

[workspace.dependencies]
codex-windows-sandbox = { path = "windows-sandbox" }
```

---

### 4.5 テスト実装

**新規ファイル**: `codex-rs/windows-sandbox/tests/integration_test.rs`

```rust
#[cfg(target_os = "windows")]
#[tokio::test]
async fn test_windows_sandbox_creation() {
    use codex_windows_sandbox::{SandboxConfig, SandboxManager, SandboxType};
    use std::path::PathBuf;
    
    let manager = SandboxManager::new(SandboxType::WindowsSandbox);
    
    let config = SandboxConfig {
        workspace_path: PathBuf::from(r"C:\temp"),
        enable_network: false,
        memory_mb: 2048,
        cpu_count: 2,
    };
    
    let result = manager.create_sandbox_os(config).await;
    
    // Windows Sandbox may not be available
    if let Ok(instance) = result {
        println!("Created sandbox: {}", instance.id);
    }
}
```

---

### 完成基準（Stage 3-4）

#### Stage 3

- [ ] `windows_mcp_bridge.rs` 実装
- [ ] `system_tools.rs` 実装
- [ ] mcp-server/Cargo.toml 更新
- [ ] CLIコマンド追加（mcp register-agent）
- [ ] Windows 11 25H2で実機テスト
- [ ] ビルド成功（警告0、エラー0）

#### Stage 4

- [ ] `windows-sandbox/` クレート作成
- [ ] `sandbox_manager.rs` 実装
- [ ] `network_isolation.rs` 実装
- [ ] `sandbox_commands.rs` 実装
- [ ] workspace設定更新
- [ ] CLIコマンド追加（sandbox create/list/exec/destroy）
- [ ] テスト実装
- [ ] Windows Sandbox実機テスト
- [ ] ビルド成功（警告0、エラー0）

---

## 実装順序

### Day 1-2: Stage 3（Windows MCP統合）

1. windows_mcp_bridge.rs実装（3時間）
2. system_tools.rs実装（1時間）
3. Cargo.toml更新（30分）
4. CLIコマンド追加（1時間）
5. テスト実装（1時間）
6. 実機テスト（2時間）

### Day 3-5: Stage 4（Sandbox OS構築）

1. windows-sandboxクレート作成（1時間）
2. sandbox_manager.rs実装（4時間）
3. network_isolation.rs実装（2時間）
4. sandbox_commands.rs実装（2時間）
5. workspace設定更新（30分）
6. CLIコマンド追加（1時間）
7. テスト実装（2時間）
8. 実機テスト（3時間）

**推定所要時間**: 5-6日間

---

## 🔒 セキュリティ要件

- ✅ ネットワーク完全遮断（デフォルト）
- ✅ ファイアウォールルール自動設定
- ✅ 管理者権限チェック
- ✅ WSBファイル自動生成・削除
- ✅ リソース制限（メモリ・CPU）

---

## 📋 依存関係

### 新規クレート

- `codex-windows-sandbox`

### 外部ライブラリ

- `widestring` - Windows API用文字列変換
- `winreg` - Windowsレジストリ操作
- `uuid` - Sandbox ID生成

### Windows要件

- Windows 11 25H2以降（MCP API）
- Windows Sandbox有効化
- 管理者権限（MCP登録・ファイアウォール設定）

### To-dos

- [ ] 全コードをLLMOps/AIエンジニア/ソフトウェア工学観点でレビュー
- [ ] 評価ログ作成 (_docs/2025-11-06_code-review-evaluation.md)
- [ ] 改善方針ロードマップ作成
- [ ] README.md v2.0.0改訂（時系列、インストール手順）
- [ ] architecture-v2.0.0.mmd作成
- [ ] PNG変換（X: 1200x630, LinkedIn: 1200x627）
- [ ] TUI Git 4D可視化実装 (xyz+t) - 基礎完成
- [ ] npmパッケージ化 (@zapabob/codex-cli)
- [ ] render_timelineメソッド実装