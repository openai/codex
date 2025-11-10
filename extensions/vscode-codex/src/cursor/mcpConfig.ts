/**
 * Cursor MCP Configuration Manager
 * Automatically generates and manages MCP configuration for Cursor integration
 */

import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

export interface MCPServerConfig {
    command: string;
    args?: string[];
    env?: Record<string, string>;
    description?: string;
    disabled?: boolean;
}

export interface MCPConfig {
    mcpServers: Record<string, MCPServerConfig>;
}

export class MCPConfigManager {
    private configPath: string;
    private workspaceRoot: string | undefined;
    
    constructor(context: vscode.ExtensionContext) {
        // CursorのMCP設定ファイルパスを取得
        // Cursorは通常、ワークスペースルートの.cursor/mcp.jsonを使用
        this.workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        
        if (this.workspaceRoot) {
            this.configPath = path.join(this.workspaceRoot, '.cursor', 'mcp.json');
        } else {
            // フォールバック: ホームディレクトリ
            const homeDir = os.homedir();
            this.configPath = path.join(homeDir, '.cursor', 'mcp.json');
        }
    }
    
    /**
     * Cursor環境を検出
     */
    isCursorEnvironment(): boolean {
        // Cursorは独自の環境変数やプロセス名を持つ
        // VS Codeベースなので、拡張機能のコンテキストから判断
        const appName = vscode.env.appName.toLowerCase();
        return appName.includes('cursor');
    }
    
    /**
     * MCP設定ファイルを生成
     */
    async generateConfig(): Promise<void> {
        if (!this.isCursorEnvironment()) {
            console.log('Not running in Cursor environment, skipping MCP config generation');
            return;
        }
        
        const configDir = path.dirname(this.configPath);
        
        // ディレクトリが存在しない場合は作成
        if (!fs.existsSync(configDir)) {
            fs.mkdirSync(configDir, { recursive: true });
        }
        
        // 既存の設定を読み込み（存在する場合）
        let existingConfig: MCPConfig = { mcpServers: {} };
        if (fs.existsSync(this.configPath)) {
            try {
                const content = fs.readFileSync(this.configPath, 'utf-8');
                existingConfig = JSON.parse(content);
            } catch (error) {
                console.error('Failed to parse existing MCP config:', error);
                // 既存の設定が壊れている場合は新規作成
                existingConfig = { mcpServers: {} };
            }
        }
        
        // Codex MCPサーバー設定を追加/更新
        const codexConfig: MCPServerConfig = {
            command: 'codex',
            args: ['mcp-server'],
            env: {},
            description: 'Codex Multi-Agent System with Deep Research, Sub-Agents, and Blueprint Mode',
            disabled: false
        };
        
        existingConfig.mcpServers['codex'] = codexConfig;
        
        // 設定ファイルを書き込み
        const configJson = JSON.stringify(existingConfig, null, 2);
        fs.writeFileSync(this.configPath, configJson, 'utf-8');
        
        console.log(`✅ MCP configuration generated: ${this.configPath}`);
        
        // 通知を表示
        vscode.window.showInformationMessage(
            `✅ Codex MCP configuration updated at ${this.configPath}`,
            'Open Config'
        ).then(selection => {
            if (selection === 'Open Config') {
                vscode.workspace.openTextDocument(this.configPath).then(doc => {
                    vscode.window.showTextDocument(doc);
                });
            }
        });
    }
    
    /**
     * MCP設定ファイルのパスを取得
     */
    getConfigPath(): string {
        return this.configPath;
    }
    
    /**
     * MCP設定ファイルが存在するか確認
     */
    configExists(): boolean {
        return fs.existsSync(this.configPath);
    }
    
    /**
     * MCP設定を読み込み
     */
    async loadConfig(): Promise<MCPConfig | null> {
        if (!this.configExists()) {
            return null;
        }
        
        try {
            const content = fs.readFileSync(this.configPath, 'utf-8');
            return JSON.parse(content) as MCPConfig;
        } catch (error) {
            console.error('Failed to load MCP config:', error);
            return null;
        }
    }
    
    /**
     * Codex MCPサーバーが設定されているか確認
     */
    async isCodexConfigured(): Promise<boolean> {
        const config = await this.loadConfig();
        if (!config) {
            return false;
        }
        
        return 'codex' in config.mcpServers && !config.mcpServers['codex'].disabled;
    }
}

