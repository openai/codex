/**
 * Blueprint status badge for VS Code status bar
 */

import * as vscode from 'vscode';
import { BlueprintState } from './state';

export class BlueprintStatusBadge {
    private statusBarItem: vscode.StatusBarItem;
    
    constructor() {
        this.statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            100
        );
        this.statusBarItem.command = 'codex.blueprint.toggle';
        this.hide();
    }
    
    /**
     * Update badge with blueprint state
     */
    updateState(state: BlueprintState, blueprintId: string): void {
        const icon = this.getStateIcon(state);
        const color = this.getStateColor(state);
        const text = `$(${icon}) Blueprint: ${state}`;
        
        this.statusBarItem.text = text;
        this.statusBarItem.backgroundColor = this.getBackgroundColor(state);
        this.statusBarItem.tooltip = `Blueprint ${blueprintId} - ${state}`;
        this.show();
    }
    
    /**
     * Show "Enter Blueprint Mode" message
     */
    showInactive(): void {
        this.statusBarItem.text = '$(edit) Enter Blueprint Mode';
        this.statusBarItem.backgroundColor = undefined;
        this.statusBarItem.tooltip = 'Click to enter Blueprint Mode';
        this.show();
    }
    
    /**
     * Hide badge
     */
    hide(): void {
        this.statusBarItem.hide();
    }
    
    /**
     * Show badge
     */
    show(): void {
        this.statusBarItem.show();
    }
    
    /**
     * Dispose badge
     */
    dispose(): void {
        this.statusBarItem.dispose();
    }
    
    /**
     * Get icon for state
     */
    private getStateIcon(state: BlueprintState): string {
        switch (state) {
            case BlueprintState.Pending:
                return 'clock';
            case BlueprintState.Approved:
                return 'check';
            case BlueprintState.Rejected:
                return 'x';
            case BlueprintState.Superseded:
                return 'sync';
            case BlueprintState.Drafting:
                return 'edit';
            default:
                return 'file';
        }
    }
    
    /**
     * Get color for state
     */
    private getStateColor(state: BlueprintState): string {
        switch (state) {
            case BlueprintState.Pending:
                return 'orange';
            case BlueprintState.Approved:
                return 'green';
            case BlueprintState.Rejected:
                return 'red';
            case BlueprintState.Superseded:
                return 'gray';
            case BlueprintState.Drafting:
                return 'blue';
            default:
                return 'gray';
        }
    }
    
    /**
     * Get background color for state
     */
    private getBackgroundColor(state: BlueprintState): vscode.ThemeColor | undefined {
        switch (state) {
            case BlueprintState.Pending:
                return new vscode.ThemeColor('statusBarItem.warningBackground');
            case BlueprintState.Rejected:
                return new vscode.ThemeColor('statusBarItem.errorBackground');
            default:
                return undefined;
        }
    }
}

