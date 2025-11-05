/**
 * VS Code extension for Codex Sub-Agents and Deep Research
 */

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as yaml from 'yaml';

export function activate(context: vscode.ExtensionContext) {
    console.log('Codex Sub-Agents extension activated');

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('codex.delegateAgent', delegateAgent),
        vscode.commands.registerCommand('codex.deepResearch', deepResearch),
        vscode.commands.registerCommand('codex.listAgents', listAgents),
        vscode.commands.registerCommand('codex.reviewCode', reviewCode)
    );

    // Register tree data providers
    const agentsProvider = new AgentsTreeDataProvider();
    vscode.window.registerTreeDataProvider('codex.agentsList', agentsProvider);

    const statusProvider = new AgentStatusProvider();
    vscode.window.registerTreeDataProvider('codex.agentStatus', statusProvider);
}

export function deactivate() {
    console.log('Codex Sub-Agents extension deactivated');
}

/**
 * Delegate task to a sub-agent
 */
async function delegateAgent() {
    const agents = await loadAvailableAgents();
    
    if (agents.length === 0) {
        vscode.window.showWarningMessage('No agents found in .codex/agents/');
        return;
    }

    const selected = await vscode.window.showQuickPick(
        agents.map(a => ({ label: a.name, description: a.goal })),
        { placeHolder: 'Select an agent to delegate to' }
    );

    if (!selected) {
        return;
    }

    const goal = await vscode.window.showInputBox({
        prompt: 'Enter task goal',
        placeHolder: 'e.g., Generate tests for selected files'
    });

    if (!goal) {
        return;
    }

    vscode.window.showInformationMessage(
        `🤖 Delegating to ${selected.label}...`
    );

    // Execute codex delegate command
    const terminal = vscode.window.createTerminal('Codex Agent');
    terminal.show();
    terminal.sendText(`codex delegate ${agents.find(a => a.name === selected.label)?.id} --goal "${goal}"`);
}

/**
 * Execute deep research
 */
async function deepResearch() {
    const topic = await vscode.window.showInputBox({
        prompt: 'Enter research topic',
        placeHolder: 'e.g., Rust async patterns 2023-2025'
    });

    if (!topic) {
        return;
    }

    const depth = await vscode.window.showQuickPick(
        ['1', '2', '3', '4', '5'],
        { placeHolder: 'Select research depth (default: 3)' }
    );

    vscode.window.showInformationMessage(
        `🔍 Starting deep research: ${topic}`
    );

    const terminal = vscode.window.createTerminal('Codex Research');
    terminal.show();
    terminal.sendText(`codex research "${topic}" --depth ${depth || '3'} --breadth 8`);
}

/**
 * List available agents
 */
async function listAgents() {
    const agents = await loadAvailableAgents();

    if (agents.length === 0) {
        vscode.window.showInformationMessage('No agents configured');
        return;
    }

    const items = agents.map(a => `• ${a.name}: ${a.goal}`);
    const content = `# Available Agents\n\n${items.join('\n')}`;

    const doc = await vscode.workspace.openTextDocument({
        content,
        language: 'markdown'
    });

    vscode.window.showTextDocument(doc);
}

/**
 * Review selected code
 */
async function reviewCode() {
    const editor = vscode.window.activeTextEditor;
    
    if (!editor) {
        vscode.window.showWarningMessage('No file open');
        return;
    }

    const filePath = editor.document.uri.fsPath;
    
    vscode.window.showInformationMessage(
        `🔍 Reviewing: ${path.basename(filePath)}`
    );

    const terminal = vscode.window.createTerminal('Codex Review');
    terminal.show();
    terminal.sendText(`codex delegate code-reviewer --scope "${filePath}"`);
}

/**
 * Load available agents from .codex/agents/
 */
async function loadAvailableAgents(): Promise<AgentInfo[]> {
    const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    
    if (!workspaceRoot) {
        return [];
    }

    const agentsDir = path.join(workspaceRoot, '.codex', 'agents');

    if (!fs.existsSync(agentsDir)) {
        return [];
    }

    const files = fs.readdirSync(agentsDir);
    const agents: AgentInfo[] = [];

    for (const file of files) {
        if (file.endsWith('.yaml') || file.endsWith('.yml')) {
            try {
                const filePath = path.join(agentsDir, file);
                const content = fs.readFileSync(filePath, 'utf8');
                const data = yaml.parse(content);

                agents.push({
                    id: path.basename(file, path.extname(file)),
                    name: data.name || file,
                    goal: data.goal || 'No description',
                });
            } catch (error) {
                console.error(`Failed to load agent ${file}:`, error);
            }
        }
    }

    return agents;
}

interface AgentInfo {
    id: string;
    name: string;
    goal: string;
}

/**
 * Tree data provider for agents list
 */
class AgentsTreeDataProvider implements vscode.TreeDataProvider<AgentTreeItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<AgentTreeItem | undefined>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    refresh(): void {
        this._onDidChangeTreeData.fire(undefined);
    }

    getTreeItem(element: AgentTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: AgentTreeItem): Promise<AgentTreeItem[]> {
        if (element) {
            return [];
        }

        const agents = await loadAvailableAgents();
        return agents.map(a => new AgentTreeItem(a.name, a.goal));
    }
}

class AgentTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly description: string
    ) {
        super(label, vscode.TreeItemCollapsibleState.None);
        this.tooltip = description;
        this.iconPath = new vscode.ThemeIcon('robot');
    }
}

/**
 * Tree data provider for agent status
 */
class AgentStatusProvider implements vscode.TreeDataProvider<StatusTreeItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<StatusTreeItem | undefined>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    refresh(): void {
        this._onDidChangeTreeData.fire(undefined);
    }

    getTreeItem(element: StatusTreeItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: StatusTreeItem): Promise<StatusTreeItem[]> {
        if (element) {
            return Promise.resolve([]);
        }

        // TODO: Load actual agent status from runtime
        return Promise.resolve([
            new StatusTreeItem('No active agents', 'idle')
        ]);
    }
}

class StatusTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly status: string
    ) {
        super(label, vscode.TreeItemCollapsibleState.None);
        this.iconPath = new vscode.ThemeIcon(
            status === 'running' ? 'loading~spin' : 'circle-outline'
        );
    }
}

