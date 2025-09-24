/**
 * TurnContext implementation - ports TurnContext struct from codex-rs
 * Manages turn state, context switching, approval policies, and sandbox settings
 */

import { ModelClient } from '../models/ModelClient';
import { AskForApproval, SandboxPolicy, ReasoningEffortConfig, ReasoningSummaryConfig } from '../protocol/types';

/**
 * Shell environment policy for command execution
 */
export type ShellEnvironmentPolicy = 'preserve' | 'clean' | 'restricted';

/**
 * Tools configuration for the turn
 */
export interface ToolsConfig {
  /** Enable/disable exec_command tool */
  execCommand?: boolean;
  /** Enable/disable web_search tool */
  webSearch?: boolean;
  /** Enable/disable file operations */
  fileOperations?: boolean;
  /** Enable/disable MCP tools */
  mcpTools?: boolean;
  /** Custom tool configurations */
  customTools?: Record<string, any>;
}

/**
 * Turn configuration that can be updated during execution
 */
export interface TurnContextConfig {
  /** Current working directory */
  cwd?: string;
  /** Base instructions override */
  baseInstructions?: string;
  /** User instructions for this turn */
  userInstructions?: string;
  /** Approval policy for commands */
  approvalPolicy?: AskForApproval;
  /** Sandbox policy for tool execution */
  sandboxPolicy?: SandboxPolicy;
  /** Shell environment handling */
  shellEnvironmentPolicy?: ShellEnvironmentPolicy;
  /** Tools configuration */
  toolsConfig?: ToolsConfig;
  /** Model identifier */
  model?: string;
  /** Reasoning effort configuration */
  effort?: ReasoningEffortConfig;
  /** Reasoning summary configuration */
  summary?: ReasoningSummaryConfig;
  /** Enable review mode */
  reviewMode?: boolean;
}

/**
 * TurnContext manages the context and configuration for a single conversation turn
 * Port of TurnContext struct from codex-rs/core/src/codex.rs
 */
export class TurnContext {
  private modelClient: ModelClient;
  private cwd: string;
  private baseInstructions?: string;
  private userInstructions?: string;
  private approvalPolicy: AskForApproval;
  private sandboxPolicy: SandboxPolicy;
  private shellEnvironmentPolicy: ShellEnvironmentPolicy;
  private toolsConfig: ToolsConfig;
  private reviewMode: boolean;

  constructor(
    modelClient: ModelClient,
    config: TurnContextConfig = {}
  ) {
    this.modelClient = modelClient;

    // Initialize with defaults or provided config
    this.cwd = config.cwd || '/';
    this.baseInstructions = config.baseInstructions;
    this.userInstructions = config.userInstructions;
    this.approvalPolicy = config.approvalPolicy || 'on-request';
    this.sandboxPolicy = config.sandboxPolicy || { mode: 'workspace-write' };
    this.shellEnvironmentPolicy = config.shellEnvironmentPolicy || 'preserve';
    this.reviewMode = config.reviewMode || false;

    // Default tools configuration
    this.toolsConfig = {
      execCommand: true,
      webSearch: true,
      fileOperations: true,
      mcpTools: true,
      customTools: {},
      ...config.toolsConfig,
    };
  }

  /**
   * Update turn context configuration
   */
  update(config: TurnContextConfig): void {
    if (config.cwd !== undefined) {
      this.cwd = config.cwd;
    }
    if (config.baseInstructions !== undefined) {
      this.baseInstructions = config.baseInstructions;
    }
    if (config.userInstructions !== undefined) {
      this.userInstructions = config.userInstructions;
    }
    if (config.approvalPolicy !== undefined) {
      this.approvalPolicy = config.approvalPolicy;
    }
    if (config.sandboxPolicy !== undefined) {
      this.sandboxPolicy = config.sandboxPolicy;
    }
    if (config.shellEnvironmentPolicy !== undefined) {
      this.shellEnvironmentPolicy = config.shellEnvironmentPolicy;
    }
    if (config.toolsConfig !== undefined) {
      this.toolsConfig = { ...this.toolsConfig, ...config.toolsConfig };
    }
    if (config.reviewMode !== undefined) {
      this.reviewMode = config.reviewMode;
    }

    // Update model client if model changed
    if (config.model !== undefined) {
      this.modelClient.setModel(config.model);
    }
    if (config.effort !== undefined) {
      this.modelClient.setReasoningEffort(config.effort);
    }
    if (config.summary !== undefined) {
      this.modelClient.setReasoningSummary(config.summary);
    }
  }

  /**
   * Get current working directory
   */
  getCwd(): string {
    return this.cwd;
  }

  /**
   * Set current working directory
   */
  setCwd(cwd: string): void {
    this.cwd = cwd;
  }

  /**
   * Resolve a path relative to the current working directory
   * Port of TurnContext::resolve_path from Rust
   */
  resolvePath(path?: string): string {
    if (!path) {
      return this.cwd;
    }

    // If path is absolute, return as-is
    if (path.startsWith('/') || path.match(/^[a-zA-Z]:/)) {
      return path;
    }

    // Resolve relative path against cwd
    const resolved = this.cwd === '/'
      ? `/${path}`
      : `${this.cwd.replace(/\/$/, '')}/${path}`;

    return this.normalizePath(resolved);
  }

  /**
   * Normalize path by resolving . and .. components
   */
  private normalizePath(path: string): string {
    const parts = path.split('/').filter(part => part !== '');
    const normalized: string[] = [];

    for (const part of parts) {
      if (part === '.') {
        continue; // Skip current directory references
      } else if (part === '..') {
        normalized.pop(); // Go up one directory
      } else {
        normalized.push(part);
      }
    }

    return '/' + normalized.join('/');
  }

  /**
   * Get base instructions override
   */
  getBaseInstructions(): string | undefined {
    return this.baseInstructions;
  }

  /**
   * Set base instructions override
   */
  setBaseInstructions(instructions?: string): void {
    this.baseInstructions = instructions;
  }

  /**
   * Get user instructions
   */
  getUserInstructions(): string | undefined {
    return this.userInstructions;
  }

  /**
   * Set user instructions
   */
  setUserInstructions(instructions?: string): void {
    this.userInstructions = instructions;
  }

  /**
   * Get approval policy
   */
  getApprovalPolicy(): AskForApproval {
    return this.approvalPolicy;
  }

  /**
   * Set approval policy
   */
  setApprovalPolicy(policy: AskForApproval): void {
    this.approvalPolicy = policy;
  }

  /**
   * Check if approval is required for a command
   */
  requiresApproval(command: string, trusted: boolean = false): boolean {
    switch (this.approvalPolicy) {
      case 'never':
        return false;
      case 'untrusted':
        return !trusted;
      case 'on-failure':
        return false; // Only approve after failure
      case 'on-request':
      default:
        return true;
    }
  }

  /**
   * Get sandbox policy
   */
  getSandboxPolicy(): SandboxPolicy {
    return this.sandboxPolicy;
  }

  /**
   * Set sandbox policy
   */
  setSandboxPolicy(policy: SandboxPolicy): void {
    this.sandboxPolicy = policy;
  }

  /**
   * Check if a path is writable according to sandbox policy
   */
  isPathWritable(path: string): boolean {
    const resolvedPath = this.resolvePath(path);

    switch (this.sandboxPolicy.mode) {
      case 'danger-full-access':
        return true;

      case 'read-only':
        return false;

      case 'workspace-write':
        // Check if path is within writable roots
        const writableRoots = this.sandboxPolicy.writable_roots || [this.cwd];
        return writableRoots.some(root => {
          const normalizedRoot = this.resolvePath(root);
          return resolvedPath.startsWith(normalizedRoot);
        });

      default:
        return false;
    }
  }

  /**
   * Check if network access is allowed
   */
  isNetworkAllowed(): boolean {
    if (this.sandboxPolicy.mode === 'danger-full-access') {
      return true;
    }

    if (this.sandboxPolicy.mode === 'workspace-write') {
      return this.sandboxPolicy.network_access !== false;
    }

    return false;
  }

  /**
   * Get shell environment policy
   */
  getShellEnvironmentPolicy(): ShellEnvironmentPolicy {
    return this.shellEnvironmentPolicy;
  }

  /**
   * Set shell environment policy
   */
  setShellEnvironmentPolicy(policy: ShellEnvironmentPolicy): void {
    this.shellEnvironmentPolicy = policy;
  }

  /**
   * Get tools configuration
   */
  getToolsConfig(): ToolsConfig {
    return { ...this.toolsConfig };
  }

  /**
   * Update tools configuration
   */
  updateToolsConfig(config: Partial<ToolsConfig>): void {
    this.toolsConfig = { ...this.toolsConfig, ...config };
  }

  /**
   * Check if a specific tool is enabled
   */
  isToolEnabled(toolName: string): boolean {
    switch (toolName) {
      case 'exec_command':
        return this.toolsConfig.execCommand !== false;
      case 'web_search':
        return this.toolsConfig.webSearch !== false;
      case 'file_operations':
        return this.toolsConfig.fileOperations !== false;
      case 'mcp_tools':
        return this.toolsConfig.mcpTools !== false;
      default:
        return this.toolsConfig.customTools?.[toolName] !== false;
    }
  }

  /**
   * Get model client
   */
  getModelClient(): ModelClient {
    return this.modelClient;
  }

  /**
   * Get current model identifier
   */
  getModel(): string {
    return this.modelClient.getModel();
  }

  /**
   * Get model context window size
   */
  getModelContextWindow(): number | undefined {
    return this.modelClient.getContextWindow();
  }

  /**
   * Get reasoning effort configuration
   */
  getEffort(): ReasoningEffortConfig | undefined {
    return this.modelClient.getReasoningEffort();
  }

  /**
   * Get reasoning summary configuration
   */
  getSummary(): ReasoningSummaryConfig {
    return this.modelClient.getReasoningSummary() || { enabled: false };
  }

  /**
   * Check if in review mode
   */
  isReviewMode(): boolean {
    return this.reviewMode;
  }

  /**
   * Set review mode
   */
  setReviewMode(enabled: boolean): void {
    this.reviewMode = enabled;
  }

  /**
   * Create a copy of this turn context
   */
  clone(): TurnContext {
    const cloned = new TurnContext(this.modelClient, {
      cwd: this.cwd,
      baseInstructions: this.baseInstructions,
      userInstructions: this.userInstructions,
      approvalPolicy: this.approvalPolicy,
      sandboxPolicy: structuredClone(this.sandboxPolicy),
      shellEnvironmentPolicy: this.shellEnvironmentPolicy,
      toolsConfig: structuredClone(this.toolsConfig),
      reviewMode: this.reviewMode,
    });

    return cloned;
  }

  /**
   * Export turn context for serialization
   */
  export(): {
    cwd: string;
    baseInstructions?: string;
    userInstructions?: string;
    approvalPolicy: AskForApproval;
    sandboxPolicy: SandboxPolicy;
    shellEnvironmentPolicy: ShellEnvironmentPolicy;
    toolsConfig: ToolsConfig;
    model: string;
    effort?: ReasoningEffortConfig;
    summary: ReasoningSummaryConfig;
    reviewMode: boolean;
  } {
    return {
      cwd: this.cwd,
      baseInstructions: this.baseInstructions,
      userInstructions: this.userInstructions,
      approvalPolicy: this.approvalPolicy,
      sandboxPolicy: structuredClone(this.sandboxPolicy),
      shellEnvironmentPolicy: this.shellEnvironmentPolicy,
      toolsConfig: structuredClone(this.toolsConfig),
      model: this.getModel(),
      effort: this.getEffort(),
      summary: this.getSummary(),
      reviewMode: this.reviewMode,
    };
  }

  /**
   * Import turn context from serialized data
   */
  static import(
    modelClient: ModelClient,
    data: {
      cwd: string;
      baseInstructions?: string;
      userInstructions?: string;
      approvalPolicy: AskForApproval;
      sandboxPolicy: SandboxPolicy;
      shellEnvironmentPolicy: ShellEnvironmentPolicy;
      toolsConfig: ToolsConfig;
      model: string;
      effort?: ReasoningEffortConfig;
      summary: ReasoningSummaryConfig;
      reviewMode: boolean;
    }
  ): TurnContext {
    // Set model client configuration
    modelClient.setModel(data.model);
    if (data.effort) {
      modelClient.setReasoningEffort(data.effort);
    }
    modelClient.setReasoningSummary(data.summary);

    return new TurnContext(modelClient, {
      cwd: data.cwd,
      baseInstructions: data.baseInstructions,
      userInstructions: data.userInstructions,
      approvalPolicy: data.approvalPolicy,
      sandboxPolicy: data.sandboxPolicy,
      shellEnvironmentPolicy: data.shellEnvironmentPolicy,
      toolsConfig: data.toolsConfig,
      reviewMode: data.reviewMode,
    });
  }

  /**
   * Create a turn context for review mode
   */
  createReviewContext(
    reviewInstructions?: string
  ): TurnContext {
    const reviewContext = this.clone();
    reviewContext.setReviewMode(true);
    reviewContext.setBaseInstructions(reviewInstructions);
    reviewContext.setUserInstructions(undefined);

    return reviewContext;
  }

  /**
   * Validate turn context configuration
   */
  validate(): { valid: boolean; errors: string[] } {
    const errors: string[] = [];

    // Validate cwd
    if (!this.cwd) {
      errors.push('Current working directory is required');
    }

    // Validate model client
    if (!this.modelClient) {
      errors.push('Model client is required');
    }

    // Validate approval policy
    const validApprovalPolicies: AskForApproval[] = ['untrusted', 'on-failure', 'on-request', 'never'];
    if (!validApprovalPolicies.includes(this.approvalPolicy)) {
      errors.push(`Invalid approval policy: ${this.approvalPolicy}`);
    }

    // Validate sandbox policy
    const validSandboxModes = ['danger-full-access', 'read-only', 'workspace-write'];
    if (!validSandboxModes.includes(this.sandboxPolicy.mode)) {
      errors.push(`Invalid sandbox policy mode: ${this.sandboxPolicy.mode}`);
    }

    // Validate shell environment policy
    const validShellPolicies: ShellEnvironmentPolicy[] = ['preserve', 'clean', 'restricted'];
    if (!validShellPolicies.includes(this.shellEnvironmentPolicy)) {
      errors.push(`Invalid shell environment policy: ${this.shellEnvironmentPolicy}`);
    }

    return {
      valid: errors.length === 0,
      errors,
    };
  }

  /**
   * Get debug information about the turn context
   */
  getDebugInfo(): Record<string, any> {
    return {
      cwd: this.cwd,
      model: this.getModel(),
      approvalPolicy: this.approvalPolicy,
      sandboxPolicy: this.sandboxPolicy,
      shellEnvironmentPolicy: this.shellEnvironmentPolicy,
      toolsConfig: this.toolsConfig,
      reviewMode: this.reviewMode,
      modelContextWindow: this.getModelContextWindow(),
      hasBaseInstructions: !!this.baseInstructions,
      hasUserInstructions: !!this.userInstructions,
    };
  }
}