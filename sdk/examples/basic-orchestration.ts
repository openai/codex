/**
 * Basic orchestration example
 */

import { CodexOrchestrator } from '../src/orchestrator';

async function main() {
    const orchestrator = new CodexOrchestrator();

    try {
        console.log('🚀 Starting auto-orchestration example...\n');

        // Example 1: Simple task (should not orchestrate)
        console.log('Example 1: Simple task');
        const simple = await orchestrator.execute('Fix typo in README');
        console.log(`  Orchestrated: ${simple.wasOrchestrated}`);
        console.log(`  Summary: ${simple.executionSummary}\n`);

        // Example 2: Complex task (should orchestrate)
        console.log('Example 2: Complex task');
        const complex = await orchestrator.execute(
            'Implement user authentication with JWT, write tests, security review, and docs',
            { complexityThreshold: 0.7 }
        );
        console.log(`  Orchestrated: ${complex.wasOrchestrated}`);
        console.log(`  Agents: ${complex.agentsUsed.join(', ')}`);
        console.log(`  Summary:\n${complex.executionSummary}\n`);

        // Example 3: Custom strategy
        console.log('Example 3: Sequential strategy');
        const sequential = await orchestrator.execute(
            'Migrate database and update API',
            { strategy: 'sequential' }
        );
        console.log(`  Orchestrated: ${sequential.wasOrchestrated}`);
        console.log(`  Summary: ${sequential.executionSummary}\n`);

        // Example 4: JSON output
        console.log('Example 4: JSON output');
        const jsonResult = await orchestrator.execute(
            'Refactor codebase',
            { format: 'json' }
        );
        console.log(`  Result: ${JSON.stringify(jsonResult, null, 2)}\n`);

    } catch (error) {
        console.error('❌ Error:', error);
    } finally {
        await orchestrator.close();
        console.log('✅ Orchestrator closed');
    }
}

main().catch(console.error);

