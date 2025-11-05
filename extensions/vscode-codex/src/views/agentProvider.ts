/**
 * Tree data provider for active agents
 */

import * as vscode from 'vscode';
import type { OrchestratorClient } from '@zapabob/codex-protocol-client';
import type * as Types from '@zapabob/codex-protocol-client';

export class AgentProvider implements vscode.TreeDataProvider<AgentItem> {
    private readonly _onDidChangeTreeData: vscode.EventEmitter<AgentItem | undefined | null | void> =
        new vscode.EventEmitter<AgentItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<AgentItem | undefined | null | void> = this._onDidChangeTreeData.event;
    
    constructor(private client: OrchestratorClient) {}
    
    refresh(): void {
        this._onDidChangeTreeData.fire();
    }
    
    getTreeItem(element: AgentItem): vscode.TreeItem {
        return element;
    }
    
    async getChildren(element?: AgentItem): Promise<AgentItem[]> {
        if (element) {
            return [];
        }
        
        try {
            const response = await this.client.request<Types.AgentListResponse>('agent.list', {});
            const agents = response.agents || [];
            
            return agents.map((agent) => new AgentItem(
                agent.agent_id,
                agent.agent_type,
                agent.status
            ));
        } catch (error) {
            console.error('Failed to load agents:', error);
            return [];
        }
    }
}

class AgentItem extends vscode.TreeItem {
    constructor(
        public readonly agentId: string,
        public readonly agentType: string,
        public readonly status: string
    ) {
        super(agentId, vscode.TreeItemCollapsibleState.None);
        
        this.description = agentType;
        this.tooltip = `Status: ${status}`;
        this.iconPath = new vscode.ThemeIcon(
            status === 'active' ? 'check' : 'circle-outline'
        );
    }
}

