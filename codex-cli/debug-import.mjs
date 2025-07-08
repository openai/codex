console.log('START');
import 'ts-node/esm';
try {
  console.log('IMPORTING');
  await import('./src/utils/agent/agent-loop.ts');
} catch (e) {
  console.error('ERROR:', e, e.stack);
}
console.log('DONE');
