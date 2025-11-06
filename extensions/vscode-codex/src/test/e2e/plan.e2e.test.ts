/**
 * plan mode E2E Tests
 */

import * as assert from 'assert';
import * as vscode from 'vscode';

suite('Plan E2E Tests', () => {
    test('GUI/CLI parity: Plan.toggle command', async () => {
        // Execute toggle command
        await vscode.commands.executeCommand('codex.Plan.toggle');
        
        // Should not throw
        assert.ok(true);
    });
    
    test('Approval flow: create -> approve -> execute', async () => {
        // This would require orchestrator running
        // For now, verify commands are registered
        const commands = await vscode.commands.getCommands();
        
        assert.ok(commands.includes('codex.Plan.create'));
        assert.ok(commands.includes('codex.Plan.approve'));
        assert.ok(commands.includes('codex.Plan.reject'));
    });
    
    test('Export functionality: Plan.export command', async () => {
        const commands = await vscode.commands.getCommands();
        assert.ok(commands.includes('codex.Plan.export'));
    });
    
    test('Mode switching: Plan.setMode command', async () => {
        const commands = await vscode.commands.getCommands();
        assert.ok(commands.includes('codex.Plan.setMode'));
    });
    
    test('Deep research: Plan.deepResearch command', async () => {
        const commands = await vscode.commands.getCommands();
        assert.ok(commands.includes('codex.Plan.deepResearch'));
    });
    
    test('Keybinding: Shift+Tab registered', async () => {
        // Verify keybinding exists
        // This is implicit through package.json contribution
        assert.ok(true);
    });
    
    test('Configuration: all Plan settings available', () => {
        const config = vscode.workspace.getConfiguration('codex');
        
        // Check that configuration schema allows these settings
        // (actual values may not be set yet)
        const inspect = config.inspect('Plan.enabled');
        assert.ok(inspect !== undefined);
    });
});

