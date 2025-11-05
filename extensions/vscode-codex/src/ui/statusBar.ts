/**
 * Codex status bar implementation
 */

import * as vscode from 'vscode';

export type CodexStatus = 'running' | 'stopped' | 'error';

export class CodexStatusBar {
    private statusBarItem: vscode.StatusBarItem;
    
    constructor() {
        this.statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Right,
            100
        );
        this.statusBarItem.command = 'codex.showStatus';
        this.setStatus('stopped');
        this.statusBarItem.show();
    }
    
    /**
     * Set status
     */
    setStatus(status: CodexStatus): void {
        switch (status) {
            case 'running':
                this.statusBarItem.text = '$(check) Codex';
                this.statusBarItem.backgroundColor = undefined;
                this.statusBarItem.tooltip = 'Codex Orchestrator is running';
                break;
            case 'stopped':
                this.statusBarItem.text = '$(circle-slash) Codex';
                this.statusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
                this.statusBarItem.tooltip = 'Codex Orchestrator is stopped';
                break;
            case 'error':
                this.statusBarItem.text = '$(error) Codex';
                this.statusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
                this.statusBarItem.tooltip = 'Codex Orchestrator error';
                break;
        }
    }
    
    /**
     * Dispose status bar
     */
    dispose(): void {
        this.statusBarItem.dispose();
    }
}

