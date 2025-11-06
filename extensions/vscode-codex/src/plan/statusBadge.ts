/**
 * Plan status badge for VS Code status bar
 */

import * as vscode from 'vscode';
import { PlanState } from './state';

export class PlanStatusBadge {
    private statusBarItem: vscode.StatusBarItem;
    
    constructor() {
        this.statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            100
        );
        this.statusBarItem.command = 'codex.Plan.toggle';
        this.hide();
    }
    
    /**
     * Update badge with Plan state
     */
    updateState(state: PlanState, PlanId: string): void {
        const icon = this.getStateIcon(state);
        const color = this.getStateColor(state);
        const text = `$(${icon}) Plan: ${state}`;
        
        this.statusBarItem.text = text;
        this.statusBarItem.backgroundColor = this.getBackgroundColor(state);
        this.statusBarItem.tooltip = `Plan ${PlanId} - ${state}`;
        this.show();
    }
    
    /**
     * Show "Enter plan mode" message
     */
    showInactive(): void {
        this.statusBarItem.text = '$(edit) Enter plan mode';
        this.statusBarItem.backgroundColor = undefined;
        this.statusBarItem.tooltip = 'Click to enter plan mode';
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
    private getStateIcon(state: PlanState): string {
        switch (state) {
            case PlanState.Pending:
                return 'clock';
            case PlanState.Approved:
                return 'check';
            case PlanState.Rejected:
                return 'x';
            case PlanState.Superseded:
                return 'sync';
            case PlanState.Drafting:
                return 'edit';
            default:
                return 'file';
        }
    }
    
    /**
     * Get color for state
     */
    private getStateColor(state: PlanState): string {
        switch (state) {
            case PlanState.Pending:
                return 'orange';
            case PlanState.Approved:
                return 'green';
            case PlanState.Rejected:
                return 'red';
            case PlanState.Superseded:
                return 'gray';
            case PlanState.Drafting:
                return 'blue';
            default:
                return 'gray';
        }
    }
    
    /**
     * Get background color for state
     */
    private getBackgroundColor(state: PlanState): vscode.ThemeColor | undefined {
        switch (state) {
            case PlanState.Pending:
                return new vscode.ThemeColor('statusBarItem.warningBackground');
            case PlanState.Rejected:
                return new vscode.ThemeColor('statusBarItem.errorBackground');
            default:
                return undefined;
        }
    }
}

