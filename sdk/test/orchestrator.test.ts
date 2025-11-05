/**
 * Tests for CodexOrchestrator
 */

import { CodexOrchestrator, OrchestratedResult } from '../src/orchestrator';

describe('CodexOrchestrator', () => {
    let orchestrator: CodexOrchestrator;

    beforeEach(() => {
        orchestrator = new CodexOrchestrator();
    });

    afterEach(async () => {
        await orchestrator.close();
    });

    it('should create orchestrator instance', () => {
        expect(orchestrator).toBeInstanceOf(CodexOrchestrator);
    });

    // Note: These tests require a running Codex MCP server
    // For CI, these would be integration tests with mocked MCP responses

    it.skip('should auto-orchestrate complex tasks', async () => {
        const result = await orchestrator.execute(
            "Build authentication system with tests and security review"
        );

        expect(result.wasOrchestrated).toBe(true);
        expect(result.agentsUsed.length).toBeGreaterThan(1);
    });

    it.skip('should use normal execution for simple tasks', async () => {
        const result = await orchestrator.execute("Fix typo in file.ts");

        expect(result.wasOrchestrated).toBe(false);
    });

    it.skip('should support custom threshold', async () => {
        const result = await orchestrator.execute(
            "Implement feature",
            { complexityThreshold: 0.9 }
        );

        // With high threshold, likely won't orchestrate
        expect(result).toBeDefined();
    });

    it.skip('should support different strategies', async () => {
        const result = await orchestrator.execute(
            "Refactor codebase with tests",
            { strategy: 'sequential' }
        );

        expect(result).toBeDefined();
    });

    it.skip('should support JSON output format', async () => {
        const result = await orchestrator.execute(
            "Review code",
            { format: 'json' }
        );

        expect(result).toBeDefined();
        expect(result.executionSummary).toBeDefined();
    });

    it.skip('should stream orchestration events', async () => {
        const events: OrchestrationEvent[] = [];

        for await (const event of orchestrator.executeStream("Build app with tests")) {
            events.push(event);
        }

        expect(events.length).toBeGreaterThan(0);
        expect(events[0].type).toBeDefined();
    });
});

describe('CodexOrchestrator error handling', () => {
    it('should handle invalid codex command gracefully', async () => {
        const orchestrator = new CodexOrchestrator('invalid-command-path');

        await expect(async () => {
            await orchestrator.execute("test");
        }).rejects.toThrow();

        await orchestrator.close();
    });
});

