/**
 * Plan toolbar with GUI buttons
 */

import * as vscode from 'vscode';
import { PlanStateManager, PlanState } from './state';
import type { OrchestratorClient } from '@zapabob/codex-protocol-client';

export class PlanToolbar {
    private panel: vscode.WebviewPanel | undefined;
    
    constructor(
        private client: OrchestratorClient,
        private stateManager: PlanStateManager,
        private context: vscode.ExtensionContext
    ) {}
    
    /**
     * Show Plan toolbar panel
     */
    show(): void {
        if (this.panel) {
            this.panel.reveal();
            return;
        }
        
        this.panel = vscode.window.createWebviewPanel(
            'codexPlanToolbar',
            'plan mode',
            vscode.ViewColumn.Two,
            {
                enableScripts: true,
                retainContextWhenHidden: true
            }
        );
        
        this.panel.webview.html = this.getWebviewContent();
        
        // Handle messages from webview
        this.panel.webview.onDidReceiveMessage(
            async message => {
                switch (message.command) {
                    case 'togglePlan':
                        await vscode.commands.executeCommand('codex.Plan.toggle');
                        break;
                    case 'approve':
                        await vscode.commands.executeCommand('codex.Plan.approve');
                        break;
                    case 'reject':
                        await vscode.commands.executeCommand('codex.Plan.reject');
                        break;
                    case 'export':
                        await vscode.commands.executeCommand('codex.Plan.export');
                        break;
                    case 'setMode':
                        await vscode.commands.executeCommand('codex.Plan.setMode', message.mode);
                        break;
                }
                
                // Refresh panel
                this.refresh();
            },
            undefined,
            this.context.subscriptions
        );
        
        this.panel.onDidDispose(
            () => {
                this.panel = undefined;
            },
            undefined,
            this.context.subscriptions
        );
    }
    
    /**
     * Refresh panel content
     */
    refresh(): void {
        if (this.panel) {
            this.panel.webview.html = this.getWebviewContent();
        }
    }
    
    /**
     * Get webview HTML content
     */
    private getWebviewContent(): string {
        const Plan = this.stateManager.getCurrentPlan();
        const isActive = this.stateManager.isPlanModeActive();
        const state = Plan?.state || PlanState.Inactive;
        
        return `<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>plan mode</title>
            <style>
                body {
                    padding: 20px;
                    color: var(--vscode-foreground);
                    background-color: var(--vscode-editor-background);
                    font-family: var(--vscode-font-family);
                }
                .toolbar {
                    display: flex;
                    gap: 10px;
                    margin-bottom: 20px;
                }
                button {
                    padding: 8px 16px;
                    background-color: var(--vscode-button-background);
                    color: var(--vscode-button-foreground);
                    border: none;
                    cursor: pointer;
                    border-radius: 2px;
                }
                button:hover {
                    background-color: var(--vscode-button-hoverBackground);
                }
                button:disabled {
                    opacity: 0.5;
                    cursor: not-allowed;
                }
                .status {
                    padding: 10px;
                    margin-bottom: 20px;
                    border-left: 4px solid var(--vscode-textLink-foreground);
                    background-color: var(--vscode-editor-inactiveSelectionBackground);
                }
                .status.pending {
                    border-left-color: orange;
                }
                .status.approved {
                    border-left-color: green;
                }
                .status.rejected {
                    border-left-color: red;
                }
                select {
                    padding: 6px;
                    background-color: var(--vscode-input-background);
                    color: var(--vscode-input-foreground);
                    border: 1px solid var(--vscode-input-border);
                }
                .info {
                    margin-top: 20px;
                    padding: 10px;
                    background-color: var(--vscode-editor-inactiveSelectionBackground);
                }
            </style>
        </head>
        <body>
            <h2>plan mode</h2>
            
            ${isActive ? `
                <div class="status ${state}">
                    <strong>Status:</strong> ${state} ${Plan ? `(${Plan.id})` : ''}
                </div>
                
                <div class="toolbar">
                    <button onclick="togglePlan()">
                        ${isActive ? 'Exit Plan' : 'Enter Plan'}
                    </button>
                    <button onclick="approve()" ${state !== PlanState.Pending ? 'disabled' : ''}>
                        Approve
                    </button>
                    <button onclick="reject()" ${state !== PlanState.Pending ? 'disabled' : ''}>
                        Reject
                    </button>
                    <button onclick="exportPlan()" ${!Plan ? 'disabled' : ''}>
                        Export
                    </button>
                </div>
                
                <div class="toolbar">
                    <label>Execution Mode:</label>
                    <select id="modeSelector" onchange="setMode()">
                        <option value="single">Single</option>
                        <option value="orchestrated" selected>Orchestrated</option>
                        <option value="competition">Competition</option>
                    </select>
                </div>
                
                ${Plan ? `
                    <div class="info">
                        <h3>${Plan.title}</h3>
                        <p><strong>Goal:</strong> ${Plan.goal}</p>
                        <p><strong>Mode:</strong> ${Plan.mode}</p>
                        <p><strong>Created:</strong> ${new Date(Plan.createdAt).toLocaleString()}</p>
                    </div>
                ` : ''}
            ` : `
                <div class="toolbar">
                    <button onclick="togglePlan()">Enter plan mode</button>
                </div>
                <p>plan mode is currently inactive. Click above to enable read-only planning phase.</p>
            `}
            
            <script>
                const vscode = acquireVsCodeApi();
                
                function togglePlan() {
                    vscode.postMessage({ command: 'togglePlan' });
                }
                
                function approve() {
                    vscode.postMessage({ command: 'approve' });
                }
                
                function reject() {
                    vscode.postMessage({ command: 'reject' });
                }
                
                function exportPlan() {
                    vscode.postMessage({ command: 'export' });
                }
                
                function setMode() {
                    const mode = document.getElementById('modeSelector').value;
                    vscode.postMessage({ command: 'setMode', mode });
                }
            </script>
        </body>
        </html>`;
    }
}

