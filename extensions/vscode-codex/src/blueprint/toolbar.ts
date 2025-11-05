/**
 * Blueprint toolbar with GUI buttons
 */

import * as vscode from 'vscode';
import { BlueprintStateManager, BlueprintState } from './state';
import type { OrchestratorClient } from '@zapabob/codex-protocol-client';

export class BlueprintToolbar {
    private panel: vscode.WebviewPanel | undefined;
    
    constructor(
        private client: OrchestratorClient,
        private stateManager: BlueprintStateManager,
        private context: vscode.ExtensionContext
    ) {}
    
    /**
     * Show blueprint toolbar panel
     */
    show(): void {
        if (this.panel) {
            this.panel.reveal();
            return;
        }
        
        this.panel = vscode.window.createWebviewPanel(
            'codexBlueprintToolbar',
            'Blueprint Mode',
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
                    case 'toggleBlueprint':
                        await vscode.commands.executeCommand('codex.blueprint.toggle');
                        break;
                    case 'approve':
                        await vscode.commands.executeCommand('codex.blueprint.approve');
                        break;
                    case 'reject':
                        await vscode.commands.executeCommand('codex.blueprint.reject');
                        break;
                    case 'export':
                        await vscode.commands.executeCommand('codex.blueprint.export');
                        break;
                    case 'setMode':
                        await vscode.commands.executeCommand('codex.blueprint.setMode', message.mode);
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
        const blueprint = this.stateManager.getCurrentBlueprint();
        const isActive = this.stateManager.isBlueprintModeActive();
        const state = blueprint?.state || BlueprintState.Inactive;
        
        return `<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Blueprint Mode</title>
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
            <h2>Blueprint Mode</h2>
            
            ${isActive ? `
                <div class="status ${state}">
                    <strong>Status:</strong> ${state} ${blueprint ? `(${blueprint.id})` : ''}
                </div>
                
                <div class="toolbar">
                    <button onclick="toggleBlueprint()">
                        ${isActive ? 'Exit Blueprint' : 'Enter Blueprint'}
                    </button>
                    <button onclick="approve()" ${state !== BlueprintState.Pending ? 'disabled' : ''}>
                        Approve
                    </button>
                    <button onclick="reject()" ${state !== BlueprintState.Pending ? 'disabled' : ''}>
                        Reject
                    </button>
                    <button onclick="exportBlueprint()" ${!blueprint ? 'disabled' : ''}>
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
                
                ${blueprint ? `
                    <div class="info">
                        <h3>${blueprint.title}</h3>
                        <p><strong>Goal:</strong> ${blueprint.goal}</p>
                        <p><strong>Mode:</strong> ${blueprint.mode}</p>
                        <p><strong>Created:</strong> ${new Date(blueprint.createdAt).toLocaleString()}</p>
                    </div>
                ` : ''}
            ` : `
                <div class="toolbar">
                    <button onclick="toggleBlueprint()">Enter Blueprint Mode</button>
                </div>
                <p>Blueprint Mode is currently inactive. Click above to enable read-only planning phase.</p>
            `}
            
            <script>
                const vscode = acquireVsCodeApi();
                
                function toggleBlueprint() {
                    vscode.postMessage({ command: 'toggleBlueprint' });
                }
                
                function approve() {
                    vscode.postMessage({ command: 'approve' });
                }
                
                function reject() {
                    vscode.postMessage({ command: 'reject' });
                }
                
                function exportBlueprint() {
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

