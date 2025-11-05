/**
 * Orchestrator Manager
 * Manages lifecycle of Codex Orchestrator process
 */

import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

export interface OrchestratorStatus {
    running: boolean;
    pid?: number;
    uptime?: string;
    memory?: number;
}

export class OrchestratorManager {
    private process: child_process.ChildProcess | null = null;
    private config: vscode.WorkspaceConfiguration;
    private logDir: string;
    
    constructor(config: vscode.WorkspaceConfiguration) {
        this.config = config;
        this.logDir = path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '', '.codex', 'logs');
        
        // Ensure log directory exists
        if (!fs.existsSync(this.logDir)) {
            fs.mkdirSync(this.logDir, { recursive: true });
        }
    }
    
    async start(): Promise<void> {
        if (this.process) {
            throw new Error('Orchestrator is already running');
        }
        
        const transport = this.config.get<'tcp' | 'uds' | 'named-pipe'>('orchestrator.transport', 'tcp');
        const args = ['orchestrator', 'start'];
        
        switch (transport) {
            case 'tcp':
                const port = this.config.get('orchestrator.port', 9876);
                args.push('--transport', 'tcp', '--port', port.toString());
                break;
            case 'uds':
                const socket = this.config.get('orchestrator.socket', '/tmp/codex-orchestrator.sock');
                args.push('--transport', 'uds', '--socket', socket);
                break;
            case 'named-pipe':
                const pipe = this.config.get('orchestrator.pipe', '\\\\.\\pipe\\codex-orchestrator');
                args.push('--transport', 'named-pipe', '--pipe', pipe);
                break;
        }
        
        // Spawn process
        this.process = child_process.spawn('codex', args, {
            detached: true,
            stdio: ['ignore', 'pipe', 'pipe']
        });
        
        // Setup logging
        const timestamp = new Date().toISOString().replace(/:/g, '-').replace(/\..+/, '');
        const logFile = path.join(this.logDir, `orchestrator_${timestamp}.log`);
        const logStream = fs.createWriteStream(logFile, { flags: 'a' });
        
        this.process.stdout?.pipe(logStream);
        this.process.stderr?.pipe(logStream);
        
        // Save PID
        const pidFile = path.join(this.logDir, 'orchestrator.pid');
        fs.writeFileSync(pidFile, this.process.pid?.toString() || '');
        
        // Handle process exit
        this.process.on('exit', (code) => {
            console.log(`Orchestrator exited with code ${code}`);
            this.process = null;
            
            if (code !== 0) {
                vscode.window.showWarningMessage(`⚠️ Orchestrator exited with code ${code}`);
            }
        });
        
        // Detach process so it continues running
        this.process.unref();
        
        // Wait a bit to ensure it started
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        // Verify it's running
        const status = await this.getStatus();
        if (!status.running) {
            throw new Error('Orchestrator failed to start');
        }
    }
    
    async stop(): Promise<void> {
        const pidFile = path.join(this.logDir, 'orchestrator.pid');
        
        if (!fs.existsSync(pidFile)) {
            throw new Error('Orchestrator PID file not found');
        }
        
        const pid = parseInt(fs.readFileSync(pidFile, 'utf-8').trim());
        
        try {
            process.kill(pid, 'SIGTERM');
            
            // Wait for process to terminate
            for (let i = 0; i < 10; i++) {
                await new Promise(resolve => setTimeout(resolve, 500));
                
                try {
                    process.kill(pid, 0); // Check if process exists
                } catch {
                    // Process terminated
                    fs.unlinkSync(pidFile);
                    this.process = null;
                    return;
                }
            }
            
            // Force kill if still running
            process.kill(pid, 'SIGKILL');
            fs.unlinkSync(pidFile);
            this.process = null;
        } catch (error) {
            throw new Error(`Failed to stop orchestrator: ${error}`);
        }
    }
    
    async getStatus(): Promise<OrchestratorStatus> {
        const pidFile = path.join(this.logDir, 'orchestrator.pid');
        
        if (!fs.existsSync(pidFile)) {
            return { running: false };
        }
        
        const pid = parseInt(fs.readFileSync(pidFile, 'utf-8').trim());
        
        try {
            // Check if process is running
            process.kill(pid, 0);
            
            // Process exists, get info
            // Note: This is a simplified version
            // In production, you'd query /proc or use platform-specific APIs
            
            return {
                running: true,
                pid: pid,
                uptime: 'N/A', // TODO: Calculate from process start time
                memory: 0 // TODO: Get from process info
            };
        } catch {
            // Process doesn't exist
            fs.unlinkSync(pidFile);
            return { running: false };
        }
    }
    
    dispose() {
        // Don't stop orchestrator on dispose - let it keep running
        this.process = null;
    }
}


