use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use std::time::Duration;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
struct AgentMessage {
    agent_instance_id: String,
    content: String,
    requires_user_input: bool,
    agent_type: String,
}

#[derive(Debug, Clone, Serialize)]
struct UserMessage {
    agent_instance_id: String,
    content: String,
    mark_as_read: bool,
}

#[derive(Debug, Deserialize)]
struct AgentMessageResponse {
    success: bool,
    agent_instance_id: String,
    message_id: String,
    queued_user_messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PendingMessage {
    id: String,
    content: String,
    sender_type: String,
    created_at: String,
    requires_user_input: bool,
}

#[derive(Debug, Deserialize)]
struct PendingMessagesResponse {
    agent_instance_id: String,
    messages: Vec<PendingMessage>,
    status: String,
}

#[derive(Clone)]
pub struct OmnaraClient {
    client: Client,
    api_key: String,
    api_url: String,
    session_id: String,
    polling_active: Arc<AtomicBool>,
    log_path: PathBuf,
    last_agent_message_id: Arc<Mutex<Option<String>>>,
}

impl OmnaraClient {
    pub fn new(api_key: String, api_url: Option<String>, session_id: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());
        
        let api_url = api_url.unwrap_or_else(|| "https://agent-dashboard-mcp.onrender.com".to_string());
        let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        
        // Create log directory and file path
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let log_dir = PathBuf::from(home_dir).join(".omnara").join("codex_wrapper");
        let _ = fs::create_dir_all(&log_dir);
        let log_path = log_dir.join(format!("{}.log", session_id));
        
        let client_instance = Self {
            client,
            api_key: api_key.clone(),
            api_url: api_url.clone(),
            session_id: session_id.clone(),
            polling_active: Arc::new(AtomicBool::new(false)),
            log_path,
            last_agent_message_id: Arc::new(Mutex::new(None)),
        };
        
        // Log initialization
        client_instance.log(&format!(
            "=== OMNARA CLIENT INITIALIZED ===\nTime: {}\nSession ID: {}\nAPI URL: {}\nAPI Key: {}...\n",
            Utc::now().to_rfc3339(),
            session_id,
            api_url,
            if api_key.len() > 8 { &api_key[..8] } else { &api_key }
        ));
        
        client_instance
    }
    
    fn log(&self, message: &str) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let _ = writeln!(file, "{}", message);
        }
    }
    
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    
    /// Send an agent message to Omnara and return the message ID
    pub async fn send_message(&self, content: String, requires_input: bool) -> Result<String, String> {
        let message = AgentMessage {
            agent_instance_id: self.session_id.clone(),
            content: content.clone(),
            requires_user_input: requires_input,
            agent_type: "codex".to_string(),
        };
        
        let url = format!("{}/api/v1/messages/agent", self.api_url);
        
        self.log(&format!(
            "\n--- SENDING AGENT MESSAGE ---\nTime: {}\nURL: {}\nRequires Input: {}\nContent: {}\nPayload: {}\n",
            Utc::now().to_rfc3339(),
            url,
            requires_input,
            content,
            serde_json::to_string_pretty(&message).unwrap_or_else(|_| "Failed to serialize".to_string())
        ));
        
        match self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                
                if status.is_success() {
                    match response.json::<AgentMessageResponse>().await {
                        Ok(resp) => {
                            // Store the message ID for later use (using async lock)
                            {
                                let mut guard = self.last_agent_message_id.lock().await;
                                *guard = Some(resp.message_id.clone());
                            }
                            
                            self.log(&format!(
                                "Response Status: {}\nMessage ID: {}\n✓ Message sent successfully\n",
                                status,
                                resp.message_id
                            ));
                            
                            // DEBUG: Log what message we sent and got ID for
                            tracing::info!("DEBUG: Sent agent message '{}' with ID: {}", 
                                if content.len() > 50 { &content[..50] } else { &content },
                                resp.message_id
                            );
                            
                            Ok(resp.message_id)
                        }
                        Err(e) => {
                            let error = format!("Failed to parse response: {}", e);
                            self.log(&format!("✗ Error: {}\n", error));
                            Err(error)
                        }
                    }
                } else {
                    let body = response.text().await.unwrap_or_else(|_| "Failed to read body".to_string());
                    let error = format!("Omnara API error: {} - {}", status, body);
                    self.log(&format!("✗ Error: {}\n", error));
                    Err(error)
                }
            }
            Err(e) => {
                let error = format!("Failed to send to Omnara: {}", e);
                self.log(&format!("✗ Network Error: {}\n", error));
                Err(error)
            }
        }
    }
    
    /// Send a user message to Omnara
    pub async fn send_user_message(&self, content: String) -> Result<(), String> {
        let message = UserMessage {
            agent_instance_id: self.session_id.clone(),
            content: content.clone(),
            mark_as_read: true,
        };
        
        let url = format!("{}/api/v1/messages/user", self.api_url);
        
        self.log(&format!(
            "\n--- SENDING USER MESSAGE ---\nTime: {}\nURL: {}\nContent: {}\nPayload: {}\n",
            Utc::now().to_rfc3339(),
            url,
            content,
            serde_json::to_string_pretty(&message).unwrap_or_else(|_| "Failed to serialize".to_string())
        ));
        
        match self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_else(|_| "Failed to read body".to_string());
                
                self.log(&format!(
                    "Response Status: {}\nResponse Body: {}\n",
                    status,
                    body
                ));
                
                if status.is_success() {
                    self.log("✓ User message sent successfully\n");
                    Ok(())
                } else {
                    let error = format!("Omnara API error: {} - {}", status, body);
                    self.log(&format!("✗ Error: {}\n", error));
                    Err(error)
                }
            }
            Err(e) => {
                let error = format!("Failed to send user message to Omnara: {}", e);
                self.log(&format!("✗ Network Error: {}\n", error));
                Err(error)
            }
        }
    }
    
    /// Request user input for a previously sent message
    pub async fn request_user_input(&self, message_id: String) -> Result<(), String> {
        let url = format!("{}/api/v1/messages/{}/request-input", self.api_url, message_id);
        
        self.log(&format!(
            "\n--- REQUESTING USER INPUT ---\nTime: {}\nURL: {}\nMessage ID: {}\n",
            Utc::now().to_rfc3339(),
            url,
            message_id
        ));
        
        match self.client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_else(|_| "Failed to read body".to_string());
                
                self.log(&format!(
                    "Response Status: {}\nResponse Body: {}\n",
                    status,
                    body
                ));
                
                if status.is_success() {
                    self.log("✓ User input requested successfully\n");
                    Ok(())
                } else {
                    let error = format!("Omnara API error: {} - {}", status, body);
                    self.log(&format!("✗ Error: {}\n", error));
                    Err(error)
                }
            }
            Err(e) => {
                let error = format!("Failed to request user input: {}", e);
                self.log(&format!("✗ Network Error: {}\n", error));
                Err(error)
            }
        }
    }
    
    /// Poll Omnara for user responses
    pub async fn poll_for_user_response(&self) -> Option<String> {
        self.polling_active.store(true, Ordering::Relaxed);
        
        let url = format!("{}/api/v1/messages/pending?agent_instance_id={}", self.api_url, self.session_id);
        let mut backoff = Duration::from_millis(100);
        let max_backoff = Duration::from_secs(2);
        let timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();
        
        while self.polling_active.load(Ordering::Relaxed) && start.elapsed() < timeout {
            match self.client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        if let Ok(pending_response) = response.json::<PendingMessagesResponse>().await {
                            // Check if we got any user messages
                            for msg in pending_response.messages {
                                if msg.sender_type == "user" {
                                    self.polling_active.store(false, Ordering::Relaxed);
                                    self.log(&format!(
                                        "\n--- RECEIVED USER MESSAGE FROM POLLING ---\nTime: {}\nContent: {}\n",
                                        Utc::now().to_rfc3339(),
                                        msg.content
                                    ));
                                    return Some(msg.content);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Omnara polling error: {}", e);
                }
            }
            
            // Exponential backoff
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(max_backoff);
        }
        
        self.polling_active.store(false, Ordering::Relaxed);
        None
    }
    
    /// Cancel ongoing polling
    pub fn cancel_polling(&self) {
        self.polling_active.store(false, Ordering::Relaxed);
    }
    
    /// Check if currently polling
    pub fn is_polling(&self) -> bool {
        self.polling_active.load(Ordering::Relaxed)
    }
    
    /// Handle task completion - request input on last message and poll
    pub async fn handle_task_complete(&self) -> Option<String> {
        // Get the last message ID (using async lock)
        let message_id = {
            let guard = self.last_agent_message_id.lock().await;
            guard.clone()
        };
        
        if let Some(msg_id) = message_id {
            // DEBUG: Log which message ID we're requesting input for
            tracing::info!("DEBUG: Requesting user input for message ID: {}", msg_id);
            
            // Request user input on the last message
            if let Err(e) = self.request_user_input(msg_id.clone()).await {
                tracing::debug!("Failed to request user input: {}", e);
                return None;
            }
            
            // Poll for user response
            self.poll_for_user_response().await
        } else {
            tracing::info!("DEBUG: No agent message ID to request input for");
            None
        }
    }
}

/// Try to create an OmnaraClient from optional config values
pub fn create_omnara_client(
    api_key: Option<String>,
    api_url: Option<String>,
    session_id: Option<String>,
) -> Option<OmnaraClient> {
    api_key.map(|key| OmnaraClient::new(key, api_url, session_id))
}