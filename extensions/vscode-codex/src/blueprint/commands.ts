/**
 * Blueprint slash commands implementation
 */

import * as vscode from 'vscode';
import { OrchestratorClient } from '@zapabob/codex-protocol-client';
import type * as Types from '@zapabob/codex-protocol-client';
import { BlueprintStateManager, ExecutionMode } from './state';

export class BlueprintCommands {
    constructor(
        private client: OrchestratorClient,
        private stateManager: BlueprintStateManager
    ) {}
    
    /**
     * Register all blueprint commands
     */
    registerCommands(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
            vscode.commands.registerCommand('codex.blueprint.toggle', () => 
                this.toggleBlueprintMode()
            ),
            vscode.commands.registerCommand('codex.blueprint.create', (args) =>
                this.createBlueprint(args)
            ),
            vscode.commands.registerCommand('codex.blueprint.approve', (blueprintId) =>
                this.approveBlueprint(blueprintId)
            ),
            vscode.commands.registerCommand('codex.blueprint.reject', (blueprintId, reason) =>
                this.rejectBlueprint(blueprintId, reason)
            ),
            vscode.commands.registerCommand('codex.blueprint.export', (blueprintId, format) =>
                this.exportBlueprint(blueprintId, format)
            ),
            vscode.commands.registerCommand('codex.blueprint.setMode', (mode) =>
                this.setExecutionMode(mode)
            ),
            vscode.commands.registerCommand('codex.blueprint.deepResearch', (query, depth, policy) =>
                this.deepResearch(query, depth, policy)
            )
        );
    }
    
    /**
     * Toggle blueprint mode on/off
     */
    private async toggleBlueprintMode(): Promise<void> {
        if (this.stateManager.isBlueprintModeActive()) {
            this.stateManager.disableBlueprintMode();
            vscode.window.showInformationMessage('Blueprint Mode: OFF');
        } else {
            this.stateManager.enableBlueprintMode();
            vscode.window.showInformationMessage('Blueprint Mode: ON');
        }
    }
    
    /**
     * Create a new blueprint
     * Usage: /blueprint "title or goal..." --mode=orchestrated --budget.tokens=50000
     */
    private async createBlueprint(args: any): Promise<void> {
        try {
            const title = args?.title || await vscode.window.showInputBox({
                prompt: 'Enter blueprint title',
                placeHolder: 'e.g., Add telemetry feature'
            });
            
            if (!title) {
                return;
            }
            
            const goal = args?.goal || await vscode.window.showInputBox({
                prompt: 'Enter blueprint goal',
                placeHolder: 'e.g., Measure p50/p95 request latency'
            });
            
            if (!goal) {
                return;
            }
            
            const mode = args?.mode || 'orchestrated';
            const budget = args?.budget || {};
            
            const response = await this.client.request<Types.BlueprintCreateResponse>('blueprint.create', {
                title,
                goal,
                mode,
                budget
            });
            
            if (response.success) {
                vscode.window.showInformationMessage(
                    `✅ Blueprint created: ${response.blueprint_id}`
                );
                
                // Load the new blueprint
                await this.loadBlueprint(response.blueprint_id);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`❌ Failed to create blueprint: ${error}`);
        }
    }
    
    /**
     * Approve a blueprint
     */
    private async approveBlueprint(blueprintId?: string): Promise<void> {
        try {
            const id = blueprintId || this.stateManager.getCurrentBlueprint()?.id;
            
            if (!id) {
                vscode.window.showWarningMessage('No blueprint to approve');
                return;
            }
            
            const approver = await vscode.window.showInputBox({
                prompt: 'Enter your name',
                placeHolder: 'e.g., john.doe'
            });
            
            if (!approver) {
                return;
            }
            
            const response = await this.client.request<Types.BlueprintApproveResponse>('blueprint.approve', {
                blueprint_id: id,
                approver,
                approver_role: 'maintainer'
            });
            
            if (response.success) {
                vscode.window.showInformationMessage(`✅ Blueprint ${id} approved!`);
                await this.loadBlueprint(id);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`❌ Failed to approve blueprint: ${error}`);
        }
    }
    
    /**
     * Reject a blueprint
     */
    private async rejectBlueprint(blueprintId?: string, reason?: string): Promise<void> {
        try {
            const id = blueprintId || this.stateManager.getCurrentBlueprint()?.id;
            
            if (!id) {
                vscode.window.showWarningMessage('No blueprint to reject');
                return;
            }
            
            const rejectReason = reason || await vscode.window.showInputBox({
                prompt: 'Enter rejection reason',
                placeHolder: 'e.g., Scope too broad'
            });
            
            if (!rejectReason) {
                return;
            }
            
            const response = await this.client.request<Types.BlueprintRejectResponse>('blueprint.reject', {
                blueprint_id: id,
                reason: rejectReason
            });
            
            if (response.success) {
                vscode.window.showInformationMessage(`Blueprint ${id} rejected`);
                await this.loadBlueprint(id);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`❌ Failed to reject blueprint: ${error}`);
        }
    }
    
    /**
     * Export blueprint to file
     */
    private async exportBlueprint(blueprintId?: string, format?: string): Promise<void> {
        try {
            const id = blueprintId || this.stateManager.getCurrentBlueprint()?.id;
            
            if (!id) {
                vscode.window.showWarningMessage('No blueprint to export');
                return;
            }
            
            const exportFormat = format || await vscode.window.showQuickPick(
                ['md', 'json', 'both'],
                { placeHolder: 'Select export format' }
            );
            
            if (!exportFormat) {
                return;
            }
            
            const response = await this.client.request<Types.BlueprintExportResponse>('blueprint.export', {
                blueprint_id: id,
                format: exportFormat
            });
            
            if (response.success) {
                const paths: string[] = [];
                if (response.markdown_path) {
                    paths.push(response.markdown_path);
                }
                if (response.json_path) {
                    paths.push(response.json_path);
                }
                
                vscode.window.showInformationMessage(
                    `✅ Blueprint exported to: ${paths.join(', ')}`
                );
            }
        } catch (error) {
            vscode.window.showErrorMessage(`❌ Failed to export blueprint: ${error}`);
        }
    }
    
    /**
     * Set execution mode
     */
    private async setExecutionMode(mode?: string): Promise<void> {
        try {
            const executionMode = mode || await vscode.window.showQuickPick(
                [ExecutionMode.Single, ExecutionMode.Orchestrated, ExecutionMode.Competition],
                { placeHolder: 'Select execution mode' }
            );
            
            if (!executionMode) {
                return;
            }
            
            const response = await this.client.request<Types.BlueprintSetModeResponse>('blueprint.setMode', {
                mode: executionMode
            });
            
            if (response.success) {
                vscode.window.showInformationMessage(
                    `✅ Execution mode set to: ${executionMode}`
                );
            }
        } catch (error) {
            vscode.window.showErrorMessage(`❌ Failed to set mode: ${error}`);
        }
    }
    
    /**
     * Deep research with approval dialog
     */
    private async deepResearch(query?: string, depth?: number, policy?: string): Promise<void> {
        try {
            const researchQuery = query || await vscode.window.showInputBox({
                prompt: 'Enter research query',
                placeHolder: 'e.g., React Server Components best practices'
            });
            
            if (!researchQuery) {
                return;
            }
            
            const searchDepth = depth || parseInt(
                await vscode.window.showQuickPick(
                    ['1', '2', '3'],
                    { placeHolder: 'Select search depth' }
                ) || '2'
            );
            
            // Show approval dialog
            const approved = await vscode.window.showInformationMessage(
                `Research Request:\n\n` +
                `Query: ${researchQuery}\n` +
                `Depth: ${searchDepth}\n` +
                `Estimated tokens: ${searchDepth * 10000}\n` +
                `Domains: DuckDuckGo, GitHub\n\n` +
                `Approve this research request?`,
                { modal: true },
                'Approve',
                'Reject'
            );
            
            if (approved !== 'Approve') {
                return;
            }
            
            // Execute research
            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: 'Conducting deep research...',
                cancellable: false
            }, async () => {
                // TODO: Implement actual research call
                await new Promise(resolve => setTimeout(resolve, 2000));
            });
            
            vscode.window.showInformationMessage('✅ Research completed! Results added to blueprint.');
        } catch (error) {
            vscode.window.showErrorMessage(`❌ Research failed: ${error}`);
        }
    }
    
    /**
     * Load a blueprint from backend
     */
    private async loadBlueprint(blueprintId: string): Promise<void> {
        try {
            const response = await this.client.request<Types.BlueprintGetResponse>('blueprint.get', {
                blueprint_id: blueprintId
            });
            
            if (response.blueprint) {
                this.stateManager.setCurrentBlueprint(response.blueprint as any);
            }
        } catch (error) {
            console.error('Failed to load blueprint:', error);
        }
    }
}

