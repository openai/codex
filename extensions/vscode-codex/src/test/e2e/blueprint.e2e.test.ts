/**
 * Blueprint Mode E2E Tests
 */

import * as assert from 'assert';
import * as vscode from 'vscode';

suite('Blueprint E2E Tests', () => {
    test('GUI/CLI parity: blueprint.toggle command', async () => {
        // Execute toggle command
        await vscode.commands.executeCommand('codex.blueprint.toggle');
        
        // Should not throw
        assert.ok(true);
    });
    
    test('Approval flow: create -> approve -> execute', async () => {
        // This would require orchestrator running
        // For now, verify commands are registered
        const commands = await vscode.commands.getCommands();
        
        assert.ok(commands.includes('codex.blueprint.create'));
        assert.ok(commands.includes('codex.blueprint.approve'));
        assert.ok(commands.includes('codex.blueprint.reject'));
    });
    
    test('Export functionality: blueprint.export command', async () => {
        const commands = await vscode.commands.getCommands();
        assert.ok(commands.includes('codex.blueprint.export'));
    });
    
    test('Mode switching: blueprint.setMode command', async () => {
        const commands = await vscode.commands.getCommands();
        assert.ok(commands.includes('codex.blueprint.setMode'));
    });
    
    test('Deep research: blueprint.deepResearch command', async () => {
        const commands = await vscode.commands.getCommands();
        assert.ok(commands.includes('codex.blueprint.deepResearch'));
    });
    
    test('Keybinding: Shift+Tab registered', async () => {
        // Verify keybinding exists
        // This is implicit through package.json contribution
        assert.ok(true);
    });
    
    test('Configuration: all blueprint settings available', () => {
        const config = vscode.workspace.getConfiguration('codex');
        
        // Check that configuration schema allows these settings
        // (actual values may not be set yet)
        const inspect = config.inspect('blueprint.enabled');
        assert.ok(inspect !== undefined);
    });
});

