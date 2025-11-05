/**
 * Approval dialog for deep research and privileged operations
 */

import * as vscode from 'vscode';

export interface ApprovalRequest {
    operation: string;
    query?: string;
    depth?: number;
    domains?: string[];
    tokenBudget?: number;
    timeBudgetSecs?: number;
    dataRetention?: string;
}

export class ApprovalDialog {
    /**
     * Show approval dialog for deep research
     */
    static async showResearchApproval(request: ApprovalRequest): Promise<boolean> {
        const message = [
            `Research Request:\n`,
            `Query: ${request.query || 'N/A'}`,
            `Depth: ${request.depth || 1}`,
            `Domains: ${request.domains?.join(', ') || 'Default search providers'}`,
            `Token Budget: ~${request.tokenBudget || 10000} tokens`,
            `Time Budget: ~${Math.floor((request.timeBudgetSecs || 60) / 60)} minutes`,
            `Data Retention: ${request.dataRetention || '30 days, then auto-deleted'}`,
            `\nApprove this research request?`
        ].join('\n');
        
        const result = await vscode.window.showInformationMessage(
            message,
            { modal: true },
            'Approve',
            'Reject'
        );
        
        return result === 'Approve';
    }
    
    /**
     * Show approval dialog for network operation
     */
    static async showNetworkApproval(operation: string, domain?: string): Promise<boolean> {
        const message = [
            `Network Operation Request:\n`,
            `Operation: ${operation}`,
            domain ? `Domain: ${domain}` : '',
            `\nThis operation requires network access.`,
            `Approve this request?`
        ].filter(Boolean).join('\n');
        
        const result = await vscode.window.showWarningMessage(
            message,
            { modal: true },
            'Approve',
            'Reject'
        );
        
        return result === 'Approve';
    }
    
    /**
     * Show approval dialog for package installation
     */
    static async showInstallApproval(packages: string[]): Promise<boolean> {
        const message = [
            `Package Installation Request:\n`,
            `Packages: ${packages.join(', ')}`,
            `\nThis operation will install packages.`,
            `Approve this installation?`
        ].join('\n');
        
        const result = await vscode.window.showWarningMessage(
            message,
            { modal: true },
            'Approve',
            'Reject'
        );
        
        return result === 'Approve';
    }
    
    /**
     * Show approval dialog for destructive git operation
     */
    static async showGitDestructiveApproval(operation: string): Promise<boolean> {
        const message = [
            `⚠️ Destructive Git Operation:\n`,
            `Operation: ${operation}`,
            `\nThis is a DESTRUCTIVE operation and cannot be undone.`,
            `Are you sure you want to proceed?`
        ].join('\n');
        
        const result = await vscode.window.showErrorMessage(
            message,
            { modal: true },
            'Approve',
            'Cancel'
        );
        
        return result === 'Approve';
    }
    
    /**
     * Show generic approval dialog
     */
    static async showGenericApproval(
        title: string,
        description: string,
        level: 'info' | 'warning' | 'error' = 'info'
    ): Promise<boolean> {
        const message = `${title}\n\n${description}\n\nApprove this operation?`;
        
        let result;
        switch (level) {
            case 'error':
                result = await vscode.window.showErrorMessage(
                    message,
                    { modal: true },
                    'Approve',
                    'Reject'
                );
                break;
            case 'warning':
                result = await vscode.window.showWarningMessage(
                    message,
                    { modal: true },
                    'Approve',
                    'Reject'
                );
                break;
            default:
                result = await vscode.window.showInformationMessage(
                    message,
                    { modal: true },
                    'Approve',
                    'Reject'
                );
        }
        
        return result === 'Approve';
    }
}

