/**
 * Blueprint Mode Tests
 */

import * as assert from 'assert';
import { BlueprintState, BlueprintStateManager, ExecutionMode } from '../blueprint/state';

suite('Blueprint State Management', () => {
    let stateManager: BlueprintStateManager;
    
    setup(() => {
        stateManager = new BlueprintStateManager();
    });
    
    test('should start inactive', () => {
        assert.strictEqual(stateManager.isBlueprintModeActive(), false);
        assert.strictEqual(stateManager.getCurrentBlueprint(), null);
    });
    
    test('should enable blueprint mode', () => {
        stateManager.enableBlueprintMode();
        assert.strictEqual(stateManager.isBlueprintModeActive(), true);
    });
    
    test('should disable blueprint mode', () => {
        stateManager.enableBlueprintMode();
        stateManager.disableBlueprintMode();
        assert.strictEqual(stateManager.isBlueprintModeActive(), false);
    });
    
    test('should get correct state colors', () => {
        assert.strictEqual(stateManager.getStateColor(BlueprintState.Pending), 'orange');
        assert.strictEqual(stateManager.getStateColor(BlueprintState.Approved), 'green');
        assert.strictEqual(stateManager.getStateColor(BlueprintState.Rejected), 'red');
    });
    
    test('should get correct state icons', () => {
        assert.strictEqual(stateManager.getStateIcon(BlueprintState.Drafting), '✏️');
        assert.strictEqual(stateManager.getStateIcon(BlueprintState.Approved), '✅');
        assert.strictEqual(stateManager.getStateIcon(BlueprintState.Rejected), '❌');
    });
    
    test('canExecute should require approved state', () => {
        // No blueprint
        assert.strictEqual(stateManager.canExecute(), false);
        
        // Approved blueprint
        stateManager.setCurrentBlueprint({
            id: 'test',
            title: 'Test',
            goal: 'Test goal',
            assumptions: [],
            clarifyingQuestions: [],
            approach: '',
            mode: ExecutionMode.Single,
            workItems: [],
            risks: [],
            eval: { tests: [], metrics: {} },
            budget: {},
            rollback: '',
            artifacts: [],
            state: BlueprintState.Approved,
            needApproval: true,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        });
        
        assert.strictEqual(stateManager.canExecute(), true);
    });
    
    test('canModify should only allow drafting/inactive', () => {
        // No blueprint
        assert.strictEqual(stateManager.canModify(), false);
        
        // Drafting blueprint
        stateManager.setCurrentBlueprint({
            id: 'test',
            title: 'Test',
            goal: 'Test goal',
            assumptions: [],
            clarifyingQuestions: [],
            approach: '',
            mode: ExecutionMode.Single,
            workItems: [],
            risks: [],
            eval: { tests: [], metrics: {} },
            budget: {},
            rollback: '',
            artifacts: [],
            state: BlueprintState.Drafting,
            needApproval: true,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        });
        
        assert.strictEqual(stateManager.canModify(), true);
        
        // Approved blueprint (cannot modify)
        const approved = stateManager.getCurrentBlueprint();
        if (approved) {
            approved.state = BlueprintState.Approved;
            stateManager.setCurrentBlueprint(approved);
        }
        
        assert.strictEqual(stateManager.canModify(), false);
    });
});

