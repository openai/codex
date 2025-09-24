<!--
  ApprovalDialog - Svelte component for handling approval requests
  Shows approval details, risks, and handles user decisions
-->

<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import type { ApprovalRequest, ApprovalResponse, ApprovalStatus } from '../../core/ApprovalManager';
  import type { ReviewDecision } from '../../protocol/types';

  // Component props
  export let request: ApprovalRequest | null = null;
  export let status: ApprovalStatus | null = null;
  export let visible: boolean = false;
  export let history: ApprovalResponse[] = [];

  // Component state
  let timeRemaining = 0;
  let countdownInterval: NodeJS.Timeout | null = null;
  let userReason = '';
  let modifications: Record<string, any> = {};
  let showAdvanced = false;
  let showHistory = false;

  // Event dispatcher
  const dispatch = createEventDispatcher<{
    decision: { response: ApprovalResponse };
    cancel: { requestId: string };
  }>();

  // Update countdown timer
  $: if (status?.timeRemaining !== undefined) {
    timeRemaining = Math.ceil(status.timeRemaining / 1000);
    startCountdown();
  }

  function startCountdown() {
    if (countdownInterval) {
      clearInterval(countdownInterval);
    }

    countdownInterval = setInterval(() => {
      if (timeRemaining > 0) {
        timeRemaining -= 1;
      } else {
        clearInterval(countdownInterval!);
        countdownInterval = null;
        handleTimeout();
      }
    }, 1000);
  }

  function handleTimeout() {
    if (!request) return;

    const timeoutResponse: ApprovalResponse = {
      id: request.id,
      decision: 'reject',
      timestamp: Date.now(),
      reason: 'Request timed out',
      metadata: { timeout: true },
    };

    dispatch('decision', { response: timeoutResponse });
  }

  function handleDecision(decision: ReviewDecision) {
    if (!request) return;

    const response: ApprovalResponse = {
      id: request.id,
      decision,
      timestamp: Date.now(),
      reason: userReason || getDefaultReason(decision),
      modifications: Object.keys(modifications).length > 0 ? modifications : undefined,
    };

    dispatch('decision', { response });
    resetForm();
  }

  function handleCancel() {
    if (!request) return;
    dispatch('cancel', { requestId: request.id });
    resetForm();
  }

  function resetForm() {
    userReason = '';
    modifications = {};
    showAdvanced = false;
  }

  function getDefaultReason(decision: ReviewDecision): string {
    switch (decision) {
      case 'approve': return 'User approved action';
      case 'reject': return 'User rejected action';
      case 'request_change': return 'User requested modifications';
      default: return 'User decision';
    }
  }

  function getRiskLevelClass(riskLevel: string): string {
    switch (riskLevel) {
      case 'low': return 'risk-low';
      case 'medium': return 'risk-medium';
      case 'high': return 'risk-high';
      case 'critical': return 'risk-critical';
      default: return 'risk-unknown';
    }
  }

  function getRiskLevelIcon(riskLevel: string): string {
    switch (riskLevel) {
      case 'low': return '🟢';
      case 'medium': return '🟡';
      case 'high': return '🟠';
      case 'critical': return '🔴';
      default: return '⚪';
    }
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString();
  }

  function addModification() {
    const key = prompt('Enter parameter name:');
    if (key) {
      const value = prompt(`Enter value for ${key}:`);
      if (value !== null) {
        modifications[key] = value;
        modifications = { ...modifications }; // Trigger reactivity
      }
    }
  }

  function removeModification(key: string) {
    delete modifications[key];
    modifications = { ...modifications };
  }

  onDestroy(() => {
    if (countdownInterval) {
      clearInterval(countdownInterval);
    }
  });
</script>

{#if visible && request}
  <div class="approval-dialog-overlay" on:click={handleCancel}>
    <div class="approval-dialog" on:click|stopPropagation>
      <!-- Header -->
      <div class="dialog-header">
        <h3 class="dialog-title">{request.title}</h3>
        <div class="risk-indicator {getRiskLevelClass(request.details.riskLevel)}">
          <span class="risk-icon">{getRiskLevelIcon(request.details.riskLevel)}</span>
          <span class="risk-level">{request.details.riskLevel.toUpperCase()}</span>
        </div>
      </div>

      <!-- Countdown Timer -->
      {#if timeRemaining > 0}
        <div class="countdown-timer">
          <span class="timer-icon">⏱️</span>
          <span class="timer-text">
            Time remaining: <strong>{timeRemaining}s</strong>
          </span>
        </div>
      {/if}

      <!-- Description -->
      <div class="dialog-body">
        <p class="description">{request.description}</p>

        <!-- Action Details -->
        <div class="action-details">
          <h4>Action Details</h4>

          {#if request.details.command}
            <div class="detail-item">
              <strong>Command:</strong>
              <code class="command-code">{request.details.command}</code>
            </div>
          {/if}

          {#if request.details.filePath}
            <div class="detail-item">
              <strong>File Path:</strong>
              <span class="file-path">{request.details.filePath}</span>
            </div>
          {/if}

          {#if request.details.url}
            <div class="detail-item">
              <strong>URL:</strong>
              <a href={request.details.url} target="_blank" class="url-link">
                {request.details.url}
              </a>
            </div>
          {/if}

          {#if request.details.action}
            <div class="detail-item">
              <strong>Action:</strong>
              <span class="action-type">{request.details.action}</span>
            </div>
          {/if}

          {#if request.details.impact && request.details.impact.length > 0}
            <div class="detail-item">
              <strong>Impact:</strong>
              <ul class="impact-list">
                {#each request.details.impact as impact}
                  <li class="impact-item">{impact}</li>
                {/each}
              </ul>
            </div>
          {/if}
        </div>

        <!-- Metadata -->
        {#if request.metadata}
          <div class="metadata-section">
            <h4>Context</h4>
            <div class="metadata-grid">
              <div class="metadata-item">
                <span class="metadata-label">Tool:</span>
                <span class="metadata-value">{request.metadata.toolName}</span>
              </div>
              <div class="metadata-item">
                <span class="metadata-label">Session:</span>
                <span class="metadata-value">{request.metadata.sessionId}</span>
              </div>
              <div class="metadata-item">
                <span class="metadata-label">Turn:</span>
                <span class="metadata-value">{request.metadata.turnId}</span>
              </div>
              <div class="metadata-item">
                <span class="metadata-label">Time:</span>
                <span class="metadata-value">{formatTimestamp(request.metadata.timestamp)}</span>
              </div>
            </div>
          </div>
        {/if}

        <!-- User Input Section -->
        <div class="user-input-section">
          <h4>Your Decision</h4>

          <div class="reason-input">
            <label for="reason">Reason (optional):</label>
            <textarea
              id="reason"
              bind:value={userReason}
              placeholder="Explain your decision..."
              rows="3"
            ></textarea>
          </div>

          <!-- Advanced Options -->
          <div class="advanced-section">
            <button
              class="toggle-advanced"
              on:click={() => showAdvanced = !showAdvanced}
            >
              {showAdvanced ? '▼' : '▶'} Advanced Options
            </button>

            {#if showAdvanced}
              <div class="modifications-section">
                <h5>Parameter Modifications</h5>
                <p class="modifications-help">
                  Modify parameters when requesting changes:
                </p>

                <div class="modifications-list">
                  {#each Object.entries(modifications) as [key, value]}
                    <div class="modification-item">
                      <span class="mod-key">{key}:</span>
                      <span class="mod-value">{value}</span>
                      <button
                        class="remove-mod"
                        on:click={() => removeModification(key)}
                        title="Remove modification"
                      >
                        ✕
                      </button>
                    </div>
                  {/each}
                </div>

                <button class="add-modification" on:click={addModification}>
                  + Add Modification
                </button>
              </div>
            {/if}
          </div>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="dialog-footer">
        <div class="primary-actions">
          <button
            class="btn btn-approve"
            on:click={() => handleDecision('approve')}
            title="Approve this action"
          >
            ✅ Approve
          </button>

          <button
            class="btn btn-reject"
            on:click={() => handleDecision('reject')}
            title="Reject this action"
          >
            ❌ Reject
          </button>

          <button
            class="btn btn-modify"
            on:click={() => handleDecision('request_change')}
            title="Request changes to this action"
            disabled={Object.keys(modifications).length === 0}
          >
            🔧 Request Changes
          </button>
        </div>

        <div class="secondary-actions">
          <button class="btn btn-cancel" on:click={handleCancel}>
            Cancel
          </button>

          <button
            class="btn btn-history"
            on:click={() => showHistory = !showHistory}
          >
            📋 History ({history.length})
          </button>
        </div>
      </div>

      <!-- History Panel -->
      {#if showHistory}
        <div class="history-panel">
          <h4>Approval History</h4>
          {#if history.length === 0}
            <p class="no-history">No approval history yet.</p>
          {:else}
            <div class="history-list">
              {#each history.slice(-5) as item}
                <div class="history-item">
                  <div class="history-header">
                    <span class="history-decision decision-{item.decision}">
                      {item.decision.toUpperCase()}
                    </span>
                    <span class="history-time">{formatTimestamp(item.timestamp)}</span>
                  </div>
                  {#if item.reason}
                    <p class="history-reason">{item.reason}</p>
                  {/if}
                  {#if item.modifications}
                    <div class="history-mods">
                      Modifications: {JSON.stringify(item.modifications)}
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .approval-dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .approval-dialog {
    background: white;
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    width: 90%;
    max-width: 600px;
    max-height: 80vh;
    overflow-y: auto;
    animation: slideIn 0.2s ease-out;
  }

  @keyframes slideIn {
    from {
      opacity: 0;
      transform: scale(0.95) translateY(-20px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 20px 10px;
    border-bottom: 1px solid #e1e5e9;
  }

  .dialog-title {
    margin: 0;
    color: #1a1a1a;
    font-size: 1.4em;
    font-weight: 600;
  }

  .risk-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border-radius: 20px;
    font-size: 0.8em;
    font-weight: 600;
  }

  .risk-low { background: #e8f5e8; color: #2d5a2d; }
  .risk-medium { background: #fff3cd; color: #856404; }
  .risk-high { background: #f8d7da; color: #721c24; }
  .risk-critical { background: #d1ecf1; color: #0c5460; }

  .countdown-timer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 20px;
    background: #fff3cd;
    border-bottom: 1px solid #ffeaa7;
    color: #856404;
    font-weight: 500;
  }

  .dialog-body {
    padding: 20px;
  }

  .description {
    margin: 0 0 20px 0;
    color: #4a5568;
    font-size: 1.1em;
    line-height: 1.5;
  }

  .action-details, .metadata-section, .user-input-section {
    margin-bottom: 24px;
  }

  .action-details h4, .metadata-section h4, .user-input-section h4 {
    margin: 0 0 12px 0;
    color: #2d3748;
    font-size: 1.1em;
    font-weight: 600;
  }

  .detail-item {
    margin-bottom: 12px;
  }

  .detail-item strong {
    display: inline-block;
    width: 100px;
    color: #4a5568;
    font-weight: 600;
  }

  .command-code {
    background: #f7fafc;
    padding: 4px 8px;
    border-radius: 4px;
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 0.9em;
    border: 1px solid #e2e8f0;
  }

  .file-path {
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 0.9em;
    color: #2b6cb0;
  }

  .url-link {
    color: #3182ce;
    text-decoration: none;
  }

  .url-link:hover {
    text-decoration: underline;
  }

  .impact-list {
    margin: 4px 0 0 0;
    padding-left: 20px;
  }

  .impact-item {
    margin-bottom: 4px;
    color: #e53e3e;
    font-weight: 500;
  }

  .metadata-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .metadata-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .metadata-label {
    font-size: 0.8em;
    color: #718096;
    font-weight: 500;
  }

  .metadata-value {
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 0.9em;
    color: #2d3748;
  }

  .reason-input {
    margin-bottom: 16px;
  }

  .reason-input label {
    display: block;
    margin-bottom: 6px;
    font-weight: 500;
    color: #4a5568;
  }

  .reason-input textarea {
    width: 100%;
    padding: 8px 12px;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.9em;
    resize: vertical;
  }

  .reason-input textarea:focus {
    outline: none;
    border-color: #3182ce;
    box-shadow: 0 0 0 3px rgba(49, 130, 206, 0.1);
  }

  .toggle-advanced {
    background: none;
    border: none;
    color: #3182ce;
    cursor: pointer;
    font-weight: 500;
    padding: 4px 0;
  }

  .toggle-advanced:hover {
    color: #2c5aa0;
  }

  .modifications-section {
    margin-top: 12px;
    padding: 16px;
    background: #f7fafc;
    border-radius: 6px;
  }

  .modifications-section h5 {
    margin: 0 0 8px 0;
    color: #2d3748;
    font-size: 1em;
  }

  .modifications-help {
    margin: 0 0 12px 0;
    font-size: 0.85em;
    color: #718096;
  }

  .modification-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
  }

  .mod-key {
    font-weight: 600;
    color: #4a5568;
  }

  .mod-value {
    flex: 1;
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 0.9em;
    color: #2d3748;
  }

  .remove-mod {
    background: #e53e3e;
    color: white;
    border: none;
    border-radius: 50%;
    width: 20px;
    height: 20px;
    cursor: pointer;
    font-size: 0.7em;
  }

  .add-modification {
    background: #3182ce;
    color: white;
    border: none;
    padding: 6px 12px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.9em;
  }

  .add-modification:hover {
    background: #2c5aa0;
  }

  .dialog-footer {
    padding: 20px;
    border-top: 1px solid #e1e5e9;
    background: #f8f9fa;
    border-radius: 0 0 12px 12px;
  }

  .primary-actions {
    display: flex;
    gap: 12px;
    margin-bottom: 12px;
  }

  .secondary-actions {
    display: flex;
    justify-content: space-between;
  }

  .btn {
    padding: 10px 16px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 500;
    font-size: 0.9em;
    transition: all 0.2s;
  }

  .btn-approve {
    background: #48bb78;
    color: white;
  }

  .btn-approve:hover {
    background: #38a169;
  }

  .btn-reject {
    background: #e53e3e;
    color: white;
  }

  .btn-reject:hover {
    background: #c53030;
  }

  .btn-modify {
    background: #ed8936;
    color: white;
  }

  .btn-modify:hover:not(:disabled) {
    background: #dd6b20;
  }

  .btn-modify:disabled {
    background: #a0aec0;
    cursor: not-allowed;
  }

  .btn-cancel, .btn-history {
    background: #e2e8f0;
    color: #4a5568;
  }

  .btn-cancel:hover, .btn-history:hover {
    background: #cbd5e0;
  }

  .history-panel {
    border-top: 1px solid #e1e5e9;
    padding: 20px;
    background: #f8f9fa;
  }

  .history-panel h4 {
    margin: 0 0 16px 0;
    color: #2d3748;
  }

  .no-history {
    margin: 0;
    color: #718096;
    font-style: italic;
  }

  .history-list {
    max-height: 200px;
    overflow-y: auto;
  }

  .history-item {
    margin-bottom: 12px;
    padding: 10px;
    background: white;
    border-radius: 4px;
    border-left: 3px solid #e2e8f0;
  }

  .history-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 4px;
  }

  .history-decision {
    font-weight: 600;
    font-size: 0.8em;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .decision-approve {
    background: #c6f6d5;
    color: #22543d;
  }

  .decision-reject {
    background: #fed7d7;
    color: #742a2a;
  }

  .decision-request_change {
    background: #feebc8;
    color: #7b341e;
  }

  .history-time {
    font-size: 0.8em;
    color: #718096;
  }

  .history-reason {
    margin: 0;
    font-size: 0.9em;
    color: #4a5568;
  }

  .history-mods {
    margin-top: 4px;
    font-size: 0.8em;
    color: #718096;
    font-family: 'Monaco', 'Menlo', monospace;
  }

  @media (max-width: 600px) {
    .approval-dialog {
      width: 95%;
      margin: 20px;
    }

    .metadata-grid {
      grid-template-columns: 1fr;
    }

    .primary-actions {
      flex-direction: column;
    }

    .secondary-actions {
      flex-direction: column;
      gap: 8px;
    }
  }
</style>