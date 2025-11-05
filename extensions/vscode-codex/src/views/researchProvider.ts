/**
 * Tree data provider for research history
 */

import * as vscode from 'vscode';
import { OrchestratorClient } from '@zapabob/codex-protocol-client';

export class ResearchProvider implements vscode.TreeDataProvider<ResearchItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<ResearchItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
    
    constructor(private client: OrchestratorClient) {}
    
    refresh(): void {
        this._onDidChangeTreeData.fire();
    }
    
    getTreeItem(element: ResearchItem): vscode.TreeItem {
        return element;
    }
    
    async getChildren(element?: ResearchItem): Promise<ResearchItem[]> {
        if (element) {
            return [];
        }
        
        // TODO: Implement actual research history loading
        return [];
    }
}

class ResearchItem extends vscode.TreeItem {
    constructor(
        public readonly query: string,
        public readonly timestamp: string,
        public readonly confidence: number
    ) {
        super(query, vscode.TreeItemCollapsibleState.None);
        
        this.description = `${(confidence * 100).toFixed(0)}%`;
        this.tooltip = `Researched: ${timestamp}`;
        this.iconPath = new vscode.ThemeIcon('search');
    }
}

