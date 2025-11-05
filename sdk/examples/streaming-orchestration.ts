/**
 * Streaming orchestration example
 */

import { CodexOrchestrator } from '../src/orchestrator';

async function main() {
    const orchestrator = new CodexOrchestrator();

    try {
        console.log('🚀 Starting streaming orchestration example...\n');

        const goal = 'Build full-stack authentication system with tests and deployment';
        console.log(`Goal: ${goal}\n`);

        console.log('Progress:');
        for await (const event of orchestrator.executeStream(goal)) {
            const timestamp = new Date(event.timestamp).toLocaleTimeString();
            console.log(`[${timestamp}] [${event.type}] ${event.message}`);
        }

        console.log('\n✅ Orchestration completed');

    } catch (error) {
        console.error('❌ Error:', error);
    } finally {
        await orchestrator.close();
    }
}

main().catch(console.error);

