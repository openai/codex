/**
 * plan mode Tests
 */

import * as assert from 'assert';
import { PlanState, PlanStateManager, ExecutionMode } from '../Plan/state';

suite('Plan State Management', () => {
    let stateManager: PlanStateManager;
    
    setup(() => {
        stateManager = new PlanStateManager();
    });
    
    test('should start inactive', () => {
        assert.strictEqual(stateManager.isPlanModeActive(), false);
        assert.strictEqual(stateManager.getCurrentPlan(), null);
    });
    
    test('should enable plan mode', () => {
        stateManager.enablePlanMode();
        assert.strictEqual(stateManager.isPlanModeActive(), true);
    });
    
    test('should disable plan mode', () => {
        stateManager.enablePlanMode();
        stateManager.disablePlanMode();
        assert.strictEqual(stateManager.isPlanModeActive(), false);
    });
    
    test('should get correct state colors', () => {
        assert.strictEqual(stateManager.getStateColor(PlanState.Pending), 'orange');
        assert.strictEqual(stateManager.getStateColor(PlanState.Approved), 'green');
        assert.strictEqual(stateManager.getStateColor(PlanState.Rejected), 'red');
    });
    
    test('should get correct state icons', () => {
        assert.strictEqual(stateManager.getStateIcon(PlanState.Drafting), '✏️');
        assert.strictEqual(stateManager.getStateIcon(PlanState.Approved), '✅');
        assert.strictEqual(stateManager.getStateIcon(PlanState.Rejected), '❌');
    });
    
    test('canExecute should require approved state', () => {
        // No Plan
        assert.strictEqual(stateManager.canExecute(), false);
        
        // Approved Plan
        stateManager.setCurrentPlan({
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
            state: PlanState.Approved,
            needApproval: true,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        });
        
        assert.strictEqual(stateManager.canExecute(), true);
    });
    
    test('canModify should only allow drafting/inactive', () => {
        // No Plan
        assert.strictEqual(stateManager.canModify(), false);
        
        // Drafting Plan
        stateManager.setCurrentPlan({
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
            state: PlanState.Drafting,
            needApproval: true,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        });
        
        assert.strictEqual(stateManager.canModify(), true);
        
        // Approved Plan (cannot modify)
        const approved = stateManager.getCurrentPlan();
        if (approved) {
            approved.state = PlanState.Approved;
            stateManager.setCurrentPlan(approved);
        }
        
        assert.strictEqual(stateManager.canModify(), false);
    });
});

