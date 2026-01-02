import { spawn } from 'child_process';

const server = {
  name: 'cora-mcp',
  version: '0.1.0',
  tools: [{
    name: 'cora_learn',
    description: 'Launch CORA interactive learning session',
    inputSchema: {
      type: 'object',
      properties: {
        module: { type: 'string', description: 'Module ID (e.g., 0.1)' }
      },
      required: ['module']
    }
  }]
};

async function handleToolCall(name: string, args: any) {
  if (name === 'cora_learn') {
    const child = spawn('python3', ['./cora', args.module], {
      stdio: ['inherit', 'pipe', 'pipe']
    });
    
    let output = '';
    child.stdout.on('data', (data) => output += data.toString());
    
    await new Promise((resolve) => child.on('close', resolve));
    
    return { content: [{ type: 'text', text: output }] };
  }
}

console.log(JSON.stringify(server));
