/**
 * Tree data provider for MCP servers
 */

import * as vscode from 'vscode';
import { OrchestratorClient } from '@zapabob/codex-protocol-client';

export class MCPProvider implements vscode.TreeDataProvider<MCPItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<MCPItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
    
    constructor(private client: OrchestratorClient) {}
    
    refresh(): void {
        this._onDidChangeTreeData.fire();
    }
    
    getTreeItem(element: MCPItem): vscode.TreeItem {
        return element;
    }
    
    async getChildren(element?: MCPItem): Promise<MCPItem[]> {
        if (element) {
            return [];
        }
        
        // TODO: Implement actual MCP server listing
        return [];
    }
}

class MCPItem extends vscode.TreeItem {
    constructor(
        public readonly serverId: string,
        public readonly status: string
    ) {
        super(serverId, vscode.TreeItemCollapsibleState.None);
        
        this.description = status;
        this.tooltip = `MCP Server: ${serverId}`;
        this.iconPath = new vscode.ThemeIcon(
            status === 'connected' ? 'plug' : 'circle-outline'
        );
    }
}

