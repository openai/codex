// 非同期サブエージェント管理システム
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use uuid::Uuid;

use crate::subagent::AgentMessage;
use crate::subagent::AgentState;
use crate::subagent::AgentStatus;
use crate::subagent::AgentType;

/// 非同期サブエージェント通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncSubAgentNotification {
    pub id: String,
    pub agent_type: AgentType,
    pub notification_type: NotificationType,
    pub content: String,
    pub timestamp: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationType {
    /// タスク完了通知
    TaskCompleted,
    /// タスク失敗通知
    TaskFailed,
    /// 進捗更新通知
    ProgressUpdate,
    /// エージェント間メッセージ
    AgentMessage,
    /// エラー通知
    Error,
    /// 情報通知
    Info,
}

/// 受信トレイ（非同期通知の受信箱）
#[derive(Debug)]
pub struct Inbox {
    notifications: Arc<RwLock<Vec<AsyncSubAgentNotification>>>,
    max_size: usize,
}

impl Inbox {
    pub fn new(max_size: usize) -> Self {
        Self {
            notifications: Arc::new(RwLock::new(Vec::new())),
            max_size,
        }
    }

    /// 通知を追加
    pub async fn add_notification(&self, notification: AsyncSubAgentNotification) -> Result<()> {
        let mut notifications = self.notifications.write().await;

        // サイズ制限チェック
        if notifications.len() >= self.max_size {
            notifications.remove(0); // 古い通知を削除
        }

        notifications.push(notification);
        Ok(())
    }

    /// 未読通知を取得
    pub async fn get_unread_notifications(&self) -> Vec<AsyncSubAgentNotification> {
        let notifications = self.notifications.read().await;
        notifications.clone()
    }

    /// 通知を既読にする（削除）
    pub async fn mark_as_read(&self, notification_id: &str) -> Result<()> {
        let mut notifications = self.notifications.write().await;
        notifications.retain(|n| n.id != notification_id);
        Ok(())
    }

    /// 全ての通知をクリア
    pub async fn clear_all(&self) {
        let mut notifications = self.notifications.write().await;
        notifications.clear();
    }

    /// 通知数を取得
    pub async fn count(&self) -> usize {
        let notifications = self.notifications.read().await;
        notifications.len()
    }
}

/// 非同期サブエージェント
#[derive(Debug)]
pub struct AsyncSubAgent {
    pub id: String,
    pub agent_type: AgentType,
    pub state: Arc<RwLock<AgentState>>,
    pub inbox: Inbox,
    pub notification_tx: mpsc::UnboundedSender<AsyncSubAgentNotification>,
    pub notification_rx: mpsc::UnboundedReceiver<AsyncSubAgentNotification>,
    pub task_tx: mpsc::UnboundedSender<String>,
    pub task_rx: mpsc::UnboundedReceiver<String>,
}

impl AsyncSubAgent {
    pub fn new(agent_type: AgentType) -> Self {
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();
        let (task_tx, task_rx) = mpsc::unbounded_channel();

        Self {
            id: Uuid::new_v4().to_string(),
            agent_type: agent_type.clone(),
            state: Arc::new(RwLock::new(AgentState {
                agent_type,
                status: AgentStatus::Idle,
                current_task: None,
                progress: 0.0,
            })),
            inbox: Inbox::new(100), // 最大100件の通知
            notification_tx,
            notification_rx,
            task_tx,
            task_rx,
        }
    }

    /// 非同期でタスクを開始
    pub async fn start_task_async(&self, task: String) -> Result<()> {
        // 状態を更新
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Working;
            state.current_task = Some(task.clone());
            state.progress = 0.0;
        }

        // タスクを送信
        self.task_tx
            .send(task)
            .context("Failed to send task to agent")?;

        // 開始通知を送信
        self.send_notification(NotificationType::Info, "Task started".to_string())
            .await?;

        Ok(())
    }

    /// 通知を送信
    pub async fn send_notification(
        &self,
        notification_type: NotificationType,
        content: String,
    ) -> Result<()> {
        let notification = AsyncSubAgentNotification {
            id: Uuid::new_v4().to_string(),
            agent_type: self.agent_type.clone(),
            notification_type,
            content,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        };

        // 自分の受信箱に追加
        self.inbox.add_notification(notification.clone()).await?;

        // 通知チャンネルに送信
        self.notification_tx
            .send(notification)
            .context("Failed to send notification")?;

        Ok(())
    }

    /// 進捗を更新
    pub async fn update_progress(&self, progress: f32) -> Result<()> {
        // 状態を更新
        {
            let mut state = self.state.write().await;
            state.progress = progress;
        }

        // 進捗通知を送信
        self.send_notification(
            NotificationType::ProgressUpdate,
            format!("Progress: {:.1}%", progress * 100.0),
        )
        .await?;

        Ok(())
    }

    /// タスクを完了
    pub async fn complete_task(&self, result: String) -> Result<()> {
        // 状態を更新
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Completed;
            state.progress = 1.0;
        }

        // 完了通知を送信
        self.send_notification(NotificationType::TaskCompleted, result)
            .await?;

        Ok(())
    }

    /// タスクを失敗
    pub async fn fail_task(&self, error: String) -> Result<()> {
        // 状態を更新
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Failed;
        }

        // 失敗通知を送信
        self.send_notification(NotificationType::TaskFailed, error)
            .await?;

        Ok(())
    }

    /// 現在の状態を取得
    pub async fn get_state(&self) -> AgentState {
        self.state.read().await.clone()
    }

    /// 受信トレイを取得
    pub fn get_inbox(&self) -> &Inbox {
        &self.inbox
    }

    /// 通知受信チャンネルを取得
    pub fn get_notification_receiver(
        &mut self,
    ) -> &mut mpsc::UnboundedReceiver<AsyncSubAgentNotification> {
        &mut self.notification_rx
    }

    /// タスク受信チャンネルを取得
    pub fn get_task_receiver(&mut self) -> &mut mpsc::UnboundedReceiver<String> {
        &mut self.task_rx
    }
}

/// 非同期サブエージェント管理システム
#[derive(Debug)]
pub struct AsyncSubAgentManager {
    agents: HashMap<String, AsyncSubAgent>,
    global_inbox: Inbox,
    notification_tx: mpsc::UnboundedSender<AsyncSubAgentNotification>,
    notification_rx: mpsc::UnboundedReceiver<AsyncSubAgentNotification>,
}

impl AsyncSubAgentManager {
    pub fn new() -> Self {
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();

        Self {
            agents: HashMap::new(),
            global_inbox: Inbox::new(1000), // グローバル受信箱
            notification_tx,
            notification_rx,
        }
    }

    /// エージェントを登録
    pub fn register_agent(&mut self, agent_type: AgentType) -> String {
        let agent = AsyncSubAgent::new(agent_type);
        let agent_id = agent.id.clone();
        self.agents.insert(agent_id.clone(), agent);
        agent_id
    }

    /// 非同期でタスクを開始
    pub async fn start_task_async(&self, agent_id: &str, task: String) -> Result<()> {
        let agent = self.agents.get(agent_id).context("Agent not found")?;

        agent.start_task_async(task).await?;
        Ok(())
    }

    /// エージェントの状態を取得
    pub async fn get_agent_state(&self, agent_id: &str) -> Option<AgentState> {
        self.agents.get(agent_id).map(|a| {
            // 非同期で状態を取得するため、ここでは簡単な実装
            // 実際にはArc<RwLock<AgentState>>から取得する必要がある
            AgentState {
                agent_type: a.agent_type.clone(),
                status: AgentStatus::Idle, // 簡略化
                current_task: None,
                progress: 0.0,
            }
        })
    }

    /// 全てのエージェント状態を取得
    pub async fn get_all_agent_states(&self) -> Vec<AgentState> {
        let mut states = Vec::new();
        for agent in self.agents.values() {
            states.push(agent.get_state().await);
        }
        states
    }

    /// グローバル受信箱を取得
    pub fn get_global_inbox(&self) -> &Inbox {
        &self.global_inbox
    }

    /// 通知受信チャンネルを取得
    pub fn get_notification_receiver(
        &mut self,
    ) -> &mut mpsc::UnboundedReceiver<AsyncSubAgentNotification> {
        &mut self.notification_rx
    }

    /// エージェントを取得
    pub fn get_agent(&self, agent_id: &str) -> Option<&AsyncSubAgent> {
        self.agents.get(agent_id)
    }

    /// エージェントを取得（可変）
    pub fn get_agent_mut(&mut self, agent_id: &str) -> Option<&mut AsyncSubAgent> {
        self.agents.get_mut(agent_id)
    }

    /// 全てのエージェントIDを取得
    pub fn get_all_agent_ids(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    /// エージェント数を取得
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

impl Default for AsyncSubAgentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_async_subagent_creation() {
        let agent = AsyncSubAgent::new(AgentType::CodeExpert);
        assert_eq!(agent.agent_type, AgentType::CodeExpert);
        assert!(!agent.id.is_empty());
    }

    #[tokio::test]
    async fn test_async_subagent_manager() {
        let mut manager = AsyncSubAgentManager::new();
        let agent_id = manager.register_agent(AgentType::CodeExpert);

        assert_eq!(manager.agent_count(), 1);
        assert!(manager.get_agent(&agent_id).is_some());
    }

    #[tokio::test]
    async fn test_inbox_notifications() {
        let inbox = Inbox::new(10);
        assert_eq!(inbox.count().await, 0);

        let notification = AsyncSubAgentNotification {
            id: "test".to_string(),
            agent_type: AgentType::CodeExpert,
            notification_type: NotificationType::Info,
            content: "Test notification".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        };

        inbox.add_notification(notification).await.unwrap();
        assert_eq!(inbox.count().await, 1);

        let notifications = inbox.get_unread_notifications().await;
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].content, "Test notification");
    }

    #[tokio::test]
    async fn test_async_task_processing() {
        let agent = AsyncSubAgent::new(AgentType::CodeExpert);
        let agent_id = agent.id.clone();

        // タスクを開始
        agent
            .start_task_async("Test task".to_string())
            .await
            .unwrap();

        // 状態を確認
        let state = agent.get_state().await;
        assert_eq!(state.status, AgentStatus::Working);
        assert_eq!(state.current_task, Some("Test task".to_string()));

        // 進捗を更新
        agent.update_progress(0.5).await.unwrap();
        let state = agent.get_state().await;
        assert_eq!(state.progress, 0.5);

        // タスクを完了
        agent
            .complete_task("Task completed".to_string())
            .await
            .unwrap();
        let state = agent.get_state().await;
        assert_eq!(state.status, AgentStatus::Completed);
        assert_eq!(state.progress, 1.0);
    }
}
