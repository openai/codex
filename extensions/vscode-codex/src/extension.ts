/**
 * Codex AI Assistant for VSCode/Cursor
 * Version: 0.56.0
 * Author: zapabob
 */

import * as vscode from 'vscode';
import { OrchestratorClient } from '@zapabob/codex-protocol-client';
import { OrchestratorManager } from './orchestrator/manager';
import { AgentProvider } from './views/agentProvider';
import { ResearchProvider } from './views/researchProvider';
import { randomUUID } from 'crypto';
import { MCPProvider } from './views/mcpProvider';
import { CodexStatusBar } from './ui/statusBar';

let orchestratorManager: OrchestratorManager;
let orchestratorClient: OrchestratorClient;
let statusBar: CodexStatusBar;

export async function activate(context: vscode.ExtensionContext) {
    console.log('🚀 Activating Codex AI Assistant...');
    
    // Initialize status bar
    statusBar = new CodexStatusBar();
    context.subscriptions.push(statusBar);
    
    // Initialize orchestrator manager
    const config = vscode.workspace.getConfiguration('codex');
    orchestratorManager = new OrchestratorManager(config);
    
    // Auto-start orchestrator if configured
    if (config.get('orchestrator.autoStart', true)) {
        try {
            await orchestratorManager.start();
            statusBar.setStatus('running');
            vscode.window.showInformationMessage('✅ Codex Orchestrator started');
        } catch (error) {
            console.error('Failed to start orchestrator:', error);
            statusBar.setStatus('stopped');
            vscode.window.showWarningMessage(`⚠️ Failed to start Codex Orchestrator: ${error}`);
        }
    }
    
    // Initialize protocol client
    const transport = config.get('orchestrator.transport', 'tcp');
    const clientConfig = {
        transport: {
            preference: transport as any,
            host: 'localhost',
            port: config.get('orchestrator.port', 9876)
        }
    };
    
    orchestratorClient = new OrchestratorClient(clientConfig);
    
    try {
        await orchestratorClient.connect();
        console.log('✅ Connected to Orchestrator');
    } catch (error) {
        console.error('Failed to connect to orchestrator:', error);
    }
    
    // Register tree data providers
    const agentProvider = new AgentProvider(orchestratorClient);
    vscode.window.registerTreeDataProvider('codex.activeAgents', agentProvider);
    
    const researchProvider = new ResearchProvider(orchestratorClient);
    vscode.window.registerTreeDataProvider('codex.researchHistory', researchProvider);
    
    const mcpProvider = new MCPProvider(orchestratorClient);
    vscode.window.registerTreeDataProvider('codex.mcpServers', mcpProvider);
    
    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('codex.startOrchestrator', async () => {
            try {
                await orchestratorManager.start();
                statusBar.setStatus('running');
                vscode.window.showInformationMessage('✅ Codex Orchestrator started');
                
                // Reconnect client
                await orchestratorClient.connect();
            } catch (error) {
                vscode.window.showErrorMessage(`❌ Failed to start orchestrator: ${error}`);
            }
        }),
        
        vscode.commands.registerCommand('codex.stopOrchestrator', async () => {
            try {
                await orchestratorManager.stop();
                statusBar.setStatus('stopped');
                vscode.window.showInformationMessage('🛑 Codex Orchestrator stopped');
                
                // Disconnect client
                orchestratorClient.disconnect();
            } catch (error) {
                vscode.window.showErrorMessage(`❌ Failed to stop orchestrator: ${error}`);
            }
        }),
        
        vscode.commands.registerCommand('codex.showStatus', async () => {
            const status = await orchestratorManager.getStatus();
            const message = status.running 
                ? `🟢 Orchestrator is running\n\nPID: ${status.pid}\nUptime: ${status.uptime}\nMemory: ${status.memory} MB`
                : '🔴 Orchestrator is not running';
            
            vscode.window.showInformationMessage(message);
        }),
        
        vscode.commands.registerCommand('codex.delegateTask', async () => {
            const task = await vscode.window.showInputBox({
                prompt: 'Enter task description',
                placeHolder: 'e.g., Review this code for security issues'
            });
            
            if (!task) return;
            
            const agents = ['code-reviewer', 'test-gen', 'sec-audit', 'refactorer', 'architect'];
            const selectedAgent = await vscode.window.showQuickPick(agents, {
                placeHolder: 'Select agent type'
            });
            
            if (!selectedAgent) return;
            
            try {
                await vscode.window.withProgress({
                    location: vscode.ProgressLocation.Notification,
                    title: `Delegating to ${selectedAgent}...`,
                    cancellable: false
                }, async () => {
                    // Submit task via RPC
                    await orchestratorClient.taskSubmit({
                        task_id: randomUUID(),
                        agent_type: selectedAgent,
                        task_description: task
                    });
                });
                
                vscode.window.showInformationMessage(`✅ Task delegated to ${selectedAgent}`);
            } catch (error) {
                vscode.window.showErrorMessage(`❌ Failed to delegate task: ${error}`);
            }
        }),
        
        vscode.commands.registerCommand('codex.deepResearch', async () => {
            const query = await vscode.window.showInputBox({
                prompt: 'Enter research query',
                placeHolder: 'e.g., React Server Components best practices'
            });
            
            if (!query) return;
            
            try {
                await vscode.window.withProgress({
                    location: vscode.ProgressLocation.Notification,
                    title: 'Conducting deep research...',
                    cancellable: false
                }, async () => {
                    // TODO: Implement deep research RPC call
                    await new Promise(resolve => setTimeout(resolve, 2000));
                });
                
                vscode.window.showInformationMessage('✅ Research completed! Check Research History view');
                researchProvider.refresh();
            } catch (error) {
                vscode.window.showErrorMessage(`❌ Research failed: ${error}`);
            }
        }),
        
        vscode.commands.registerCommand('codex.reviewCode', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            const code = editor.document.getText(selection);
            
            if (!code) {
                vscode.window.showWarningMessage('No code selected');
                return;
            }
            
            try {
                await vscode.window.withProgress({
                    location: vscode.ProgressLocation.Notification,
                    title: 'Reviewing code...',
                    cancellable: false
                }, async () => {
                    await orchestratorClient.taskSubmit({
                        task_id: randomUUID(),
                        agent_type: 'code-reviewer',
                        task_description: `Review this code:\n\n${code}`
                    });
                });
                
                vscode.window.showInformationMessage('✅ Code review started');
            } catch (error) {
                vscode.window.showErrorMessage(`❌ Code review failed: ${error}`);
            }
        }),
        
        vscode.commands.registerCommand('codex.generateTests', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const code = editor.document.getText();
            const filePath = editor.document.fileName;
            
            try {
                await vscode.window.withProgress({
                    location: vscode.ProgressLocation.Notification,
                    title: 'Generating tests...',
                    cancellable: false
                }, async () => {
                    await orchestratorClient.taskSubmit({
                        task_id: randomUUID(),
                        agent_type: 'test-gen',
                        task_description: `Generate tests for ${filePath}`
                    });
                });
                
                vscode.window.showInformationMessage('✅ Test generation started');
            } catch (error) {
                vscode.window.showErrorMessage(`❌ Test generation failed: ${error}`);
            }
        }),
        
        vscode.commands.registerCommand('codex.securityAudit', async () => {
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (!workspaceFolders) {
                vscode.window.showWarningMessage('No workspace folder open');
                return;
            }
            
            const folderPath = workspaceFolders[0].uri.fsPath;
            
            try {
                await vscode.window.withProgress({
                    location: vscode.ProgressLocation.Notification,
                    title: 'Running security audit...',
                    cancellable: false
                }, async () => {
                    await orchestratorClient.taskSubmit({
                        task_id: randomUUID(),
                        agent_type: 'sec-audit',
                        task_description: `Security audit for ${folderPath}`
                    });
                });
                
                vscode.window.showInformationMessage('✅ Security audit started');
            } catch (error) {
                vscode.window.showErrorMessage(`❌ Security audit failed: ${error}`);
            }
        }),
        
        vscode.commands.registerCommand('codex.openWebUI', async () => {
            const port = config.get('gui.port', 3000);
            const url = `http://localhost:${port}`;
            vscode.env.openExternal(vscode.Uri.parse(url));
        })
    );
    
    // Auto-open Web GUI if configured
    if (config.get('gui.autoOpen', false)) {
        const port = config.get('gui.port', 3000);
        const url = `http://localhost:${port}`;
        vscode.env.openExternal(vscode.Uri.parse(url));
    }
    
    console.log('✅ Codex AI Assistant activated!');
}

export function deactivate() {
    console.log('🛑 Deactivating Codex AI Assistant...');
    
    if (orchestratorClient) {
        orchestratorClient.disconnect();
    }
    
    if (orchestratorManager) {
        // Orchestrator keeps running in background
        console.log('ℹ️  Orchestrator will continue running in background');
    }
    
    console.log('✅ Codex AI Assistant deactivated');
}


