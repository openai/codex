// Codex API Types
// Auto-generated from Rust types - DO NOT EDIT MANUALLY

export interface Model {
  id: string;
  model: string;
  displayName: string;
  description: string;
  supportedReasoningEfforts: ReasoningEffortOption[];
  defaultReasoningEffort: ReasoningEffort;
  isDefault: boolean;
}

export interface ReasoningEffortOption {
  reasoningEffort: ReasoningEffort;
  description: string;
}

export type ReasoningEffort = "low" | "medium" | "high";

export interface Conversation {
  id: string;
  model: string;
  status: "active" | "completed" | "error";
  createdAt: Date;
  lastActivity: Date;
  messageCount: number;
  summary?: string;
}

export interface Message {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: Date;
  attachments?: FileAttachment[];
}

export interface FileAttachment {
  name: string;
  type: "image" | "file";
  url: string;
  size?: number;
}

export interface Agent {
  id: string;
  name: string;
  type: "code-reviewer" | "test-gen" | "sec-audit" | "researcher" | "performance" | "debug" | "docs";
  status: "idle" | "working" | "completed" | "error";
  description: string;
  lastUsed?: Date;
}

export interface MCPConnection {
  id: string;
  name: string;
  type: "filesystem" | "github" | "sequential-thinking" | "playwright" | "gemini" | "chrome-mcp";
  status: "connected" | "disconnected" | "error";
  url?: string;
  lastConnected?: Date;
}

export interface SecurityScan {
  id: string;
  type: "dependency" | "code" | "secrets";
  status: "running" | "completed" | "failed";
  findings: SecurityFinding[];
  startedAt: Date;
  completedAt?: Date;
}

export interface SecurityFinding {
  severity: "critical" | "high" | "medium" | "low" | "info";
  title: string;
  description: string;
  location?: {
    file: string;
    line?: number;
    column?: number;
  };
  recommendation?: string;
}

export interface ResearchResult {
  id: string;
  query: string;
  status: "searching" | "analyzing" | "completed" | "failed";
  sources: ResearchSource[];
  summary?: string;
  startedAt: Date;
  completedAt?: Date;
}

export interface ResearchSource {
  url: string;
  title: string;
  snippet: string;
  confidence: number;
  publishedAt?: Date;
}

export interface SystemMetrics {
  cpuUsage: number;
  memoryUsage: number;
  diskUsage: number;
  networkUsage?: number;
  activeProcesses: number;
  uptime: number;
}

export interface NotificationItem {
  id: string;
  type: "info" | "warning" | "error" | "success";
  title: string;
  message: string;
  timestamp: Date;
  read: boolean;
}

// API Request/Response Types
export interface APIRequest<T = any> {
  method: string;
  params?: T;
  id?: string | number;
}

export interface APIResponse<T = any> {
  id?: string | number;
  result?: T;
  error?: APIError;
}

export interface APIError {
  code: number;
  message: string;
  data?: any;
}

export interface WebSocketMessage {
  type: "conversation_update" | "agent_status" | "system_metrics" | "notification";
  data: any;
}

// Configuration Types
export interface AppConfig {
  theme: "light" | "dark" | "system";
  language: "ja" | "en";
  notifications: {
    enabled: boolean;
    sound: boolean;
    desktop: boolean;
  };
  shortcuts: {
    [key: string]: string;
  };
  api: {
    timeout: number;
    retryAttempts: number;
    baseUrl?: string;
  };
}

// Form Types
export interface LoginForm {
  method: "api-key" | "oauth";
  apiKey?: string;
  email?: string;
}

export interface NewConversationForm {
  model: string;
  initialMessage: string;
  attachments?: File[];
}

export interface AgentConfigForm {
  type: string;
  name: string;
  description?: string;
  parameters?: Record<string, any>;
}

// Component Props Types
export interface DashboardProps {
  user: User | null;
  conversations: Conversation[];
  agents: Agent[];
  metrics: SystemMetrics;
  notifications: NotificationItem[];
}

export interface User {
  id: string;
  email?: string;
  name?: string;
  avatar?: string;
  plan: "free" | "plus" | "pro";
}

// Utility Types
export type LoadingState = "idle" | "loading" | "success" | "error";

export interface AsyncState<T> {
  state: LoadingState;
  data?: T;
  error?: string;
}

// Event Types
export interface ConversationEvent {
  type: "message_added" | "status_changed" | "agent_assigned";
  conversationId: string;
  data: any;
  timestamp: Date;
}

export interface AgentEvent {
  type: "status_changed" | "task_completed" | "error_occurred";
  agentId: string;
  data: any;
  timestamp: Date;
}
