use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// サブエージェントの定義
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDefinition {
    /// エージェント名
    pub name: String,
    /// エージェントの目標
    pub goal: String,
    /// ツール権限
    pub tools: ToolPermissions,
    /// ポリシー設定
    pub policies: AgentPolicies,
    /// 成功基準
    pub success_criteria: Vec<String>,
    /// 出力アーティファクト
    pub artifacts: Vec<String>,
    /// エージェント固有の設定
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// ツール権限設定
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolPermissions {
    /// MCPツールの許可リスト
    #[serde(default)]
    pub mcp: Vec<String>,
    /// ファイルシステム権限
    #[serde(default)]
    pub fs: FsPermissions,
    /// ネットワーク権限
    #[serde(default)]
    pub net: NetPermissions,
    /// シェル実行権限
    #[serde(default)]
    pub shell: ShellPermissions,
}

/// ファイルシステム権限
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FsPermissions {
    /// 読み取り権限
    #[serde(default)]
    pub read: bool,
    /// 書き込み権限（パスのリスト）
    #[serde(default)]
    pub write: FsWritePermission,
}

/// ファイル書き込み権限
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FsWritePermission {
    /// 全体の許可フラグ
    Flag(bool),
    /// 許可されたパスのリスト
    Paths(Vec<String>),
}

impl Default for FsWritePermission {
    fn default() -> Self {
        Self::Flag(false)
    }
}

/// ネットワーク権限
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NetPermissions {
    /// 許可されたドメイン/URLパターン
    #[serde(default)]
    pub allow: Vec<String>,
}

/// シェル実行権限
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ShellPermissions {
    /// コマンドなし（空配列）
    Empty(Vec<String>),
    /// 許可されたコマンドのリスト
    Commands { exec: Vec<String> },
}

impl Default for ShellPermissions {
    fn default() -> Self {
        Self::Empty(vec![])
    }
}

/// エージェントポリシー
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPolicies {
    /// シェルポリシー（互換性のため）
    #[serde(default)]
    pub shell: Option<Vec<String>>,
    /// ネットワークポリシー（互換性のため）
    #[serde(default)]
    pub net: Option<Vec<String>>,
    /// コンテキストポリシー
    #[serde(default)]
    pub context: ContextPolicy,
    /// シークレット設定
    #[serde(default)]
    pub secrets: SecretsPolicy,
}

/// コンテキストポリシー
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextPolicy {
    /// 最大トークン数
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// 保持期間（"job", "session", "permanent"）
    #[serde(default = "default_retention")]
    pub retention: String,
}

fn default_max_tokens() -> usize {
    16000
}

fn default_retention() -> String {
    "job".to_string()
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            max_tokens: default_max_tokens(),
            retention: default_retention(),
        }
    }
}

/// シークレットポリシー
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SecretsPolicy {
    /// 出力からシークレットを除去するか
    #[serde(default)]
    pub redact: bool,
}

/// エージェント実行状態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    /// 待機中
    Pending,
    /// 実行中
    Running,
    /// 完了
    Completed,
    /// 失敗
    Failed,
    /// キャンセル
    Cancelled,
}

/// エージェント実行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// エージェント名
    pub agent_name: String,
    /// 実行ステータス
    pub status: AgentStatus,
    /// 生成されたアーティファクト
    pub artifacts: Vec<String>,
    /// 使用トークン数
    pub tokens_used: usize,
    /// 実行時間（秒）
    pub duration_secs: f64,
    /// エラーメッセージ（失敗時）
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_agent_definition_deserialize() {
        let yaml = r#"
name: "Test Agent"
goal: "Test goal"
tools:
  mcp:
    - search
    - crawler
  fs:
    read: true
    write:
      - "./artifacts"
  net:
    allow:
      - "https://*"
  shell:
    exec:
      - npm
      - cargo
policies:
  context:
    max_tokens: 24000
    retention: "job"
  secrets:
    redact: true
success_criteria:
  - "基準1"
  - "基準2"
artifacts:
  - "artifacts/output.md"
"#;

        let agent: AgentDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(agent.name, "Test Agent");
        assert_eq!(agent.goal, "Test goal");
        assert_eq!(agent.tools.mcp, vec!["search", "crawler"]);
        assert!(agent.tools.fs.read);
        assert_eq!(agent.policies.context.max_tokens, 24000);
        assert!(agent.policies.secrets.redact);
    }

    #[test]
    fn test_fs_write_permission_flag() {
        let yaml = r#"
read: true
write: true
"#;
        let fs: FsPermissions = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(fs.write, FsWritePermission::Flag(true)));
    }

    #[test]
    fn test_fs_write_permission_paths() {
        let yaml = r#"
read: true
write:
  - "./artifacts"
  - "./output"
"#;
        let fs: FsPermissions = serde_yaml::from_str(yaml).unwrap();
        if let FsWritePermission::Paths(paths) = fs.write {
            assert_eq!(paths, vec!["./artifacts", "./output"]);
        } else {
            panic!("Expected Paths variant");
        }
    }
}
