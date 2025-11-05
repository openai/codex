import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as path from 'path';

let mcpServerProcess: child_process.ChildProcess | null = null;

export function activate(context: vscode.ExtensionContext) {
    console.log('Codex Multi-Agent extension for Windsurf is now active!');

    // Deep Research Command
    const researchCommand = vscode.commands.registerCommand('codex.research', async () => {
        const topic = await vscode.window.showInputBox({
            prompt: 'Enter research topic',
            placeHolder: 'e.g., React Server Components best practices'
        });

        if (!topic) return;

        const depth = await vscode.window.showQuickPick(['1', '2', '3', '4', '5'], {
            placeHolder: 'Select research depth (default: 3)'
        }) || '3';

        vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: `Researching: ${topic}`,
            cancellable: true
        }, async (progress, token) => {
            progress.report({ increment: 0, message: 'Starting research...' });

            const workspaceFolder = vscode.workspace.workspaceFolders?.[0].uri.fsPath || '';
            const binaryPath = path.join(workspaceFolder, 'codex-rs', 'target', 'release', 'codex-tui');
            const outputPath = path.join(workspaceFolder, 'artifacts', `research-${Date.now()}.md`);

            const proc = child_process.spawn(binaryPath, [
                'research',
                topic,
                '--depth', depth,
                '--output', outputPath
            ], { cwd: workspaceFolder });

            let output = '';

            proc.stdout?.on('data', (data) => {
                const text = data.toString();
                output += text;
                progress.report({ message: text.trim() });
            });

            return new Promise<void>((resolve, reject) => {
                proc.on('close', (code) => {
                    if (code === 0) {
                        vscode.window.showInformationMessage(`Research complete! Report: ${outputPath}`);
                        vscode.workspace.openTextDocument(outputPath).then(doc => {
                            vscode.window.showTextDocument(doc);
                        });
                        resolve();
                    } else {
                        reject(new Error(`Research failed with code ${code}`));
                    }
                });

                token.onCancellationRequested(() => {
                    proc.kill();
                    reject(new Error('Research cancelled'));
                });
            });
        });
    });

    // Code Review Command
    const reviewCommand = vscode.commands.registerCommand('codex.review', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showWarningMessage('No active editor');
            return;
        }

        const filePath = editor.document.uri.fsPath;
        const workspaceFolder = vscode.workspace.workspaceFolders?.[0].uri.fsPath || '';
        const binaryPath = path.join(workspaceFolder, 'codex-rs', 'target', 'release', 'codex-tui');

        vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: 'Code Review',
            cancellable: false
        }, async (progress) => {
            progress.report({ message: 'Reviewing code...' });

            const proc = child_process.spawn(binaryPath, [
                'delegate',
                'code-reviewer',
                '--scope', filePath
            ], { cwd: workspaceFolder });

            let output = '';
            proc.stdout?.on('data', (data) => { output += data.toString(); });

            return new Promise<void>((resolve) => {
                proc.on('close', () => {
                    vscode.window.showInformationMessage('Code review complete!');
                    resolve();
                });
            });
        });
    });

    // Test Generation Command
    const testGenCommand = vscode.commands.registerCommand('codex.testGen', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return;

        const filePath = editor.document.uri.fsPath;
        const workspaceFolder = vscode.workspace.workspaceFolders?.[0].uri.fsPath || '';
        const binaryPath = path.join(workspaceFolder, 'codex-rs', 'target', 'release', 'codex-tui');

        const proc = child_process.spawn(binaryPath, [
            'delegate',
            'test-gen',
            '--scope', filePath
        ], { cwd: workspaceFolder });

        vscode.window.showInformationMessage('Generating tests...');
    });

    // Security Audit Command
    const secAuditCommand = vscode.commands.registerCommand('codex.secAudit', async () => {
        const workspaceFolder = vscode.workspace.workspaceFolders?.[0].uri.fsPath || '';
        const binaryPath = path.join(workspaceFolder, 'codex-rs', 'target', 'release', 'codex-tui');

        vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: 'Security Audit',
            cancellable: false
        }, async (progress) => {
            progress.report({ message: 'Scanning for vulnerabilities...' });

            const proc = child_process.spawn(binaryPath, [
                'delegate',
                'sec-audit',
                '--scope', workspaceFolder
            ], { cwd: workspaceFolder });

            return new Promise<void>((resolve) => {
                proc.on('close', () => {
                    vscode.window.showInformationMessage('Security audit complete!');
                    resolve();
                });
            });
        });
    });

    // Start MCP Server Command
    const startMCPCommand = vscode.commands.registerCommand('codex.startMCP', async () => {
        if (mcpServerProcess) {
            vscode.window.showWarningMessage('MCP Server is already running');
            return;
        }

        const config = vscode.workspace.getConfiguration('codex');
        const serverPath = config.get<string>('mcpServerPath');
        
        if (!serverPath) {
            vscode.window.showErrorMessage('MCP Server path not configured');
            return;
        }

        mcpServerProcess = child_process.spawn('node', [serverPath]);
        
        mcpServerProcess.stdout?.on('data', (data) => {
            console.log(`MCP Server: ${data.toString()}`);
        });

        mcpServerProcess.on('close', () => {
            mcpServerProcess = null;
            vscode.window.showInformationMessage('MCP Server stopped');
        });

        vscode.window.showInformationMessage('MCP Server started');
    });

    context.subscriptions.push(researchCommand, reviewCommand, testGenCommand, secAuditCommand, startMCPCommand);
}

export function deactivate() {
    if (mcpServerProcess) {
        mcpServerProcess.kill();
    }
}

