#!/usr/bin/env node
import { createInterface } from 'readline';

// Basic set of tools
const tools = [
  {
    name: 'echo',
    description: 'Echoes a message back',
    parameters: {
      type: 'object',
      properties: {
        message: { type: 'string' }
      },
      required: ['message']
    }
  },
  {
    name: 'add',
    description: 'Adds two numbers',
    parameters: {
      type: 'object',
      properties: {
        a: { type: 'number' },
        b: { type: 'number' }
      },
      required: ['a', 'b']
    }
  }
];

// Set up readline interface
const rl = createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false
});

// Handle incoming messages
rl.on('line', (line) => {
  try {
    const message = JSON.parse(line);
    
    // Handle different request types
    switch (message.type) {
      case 'init':
      case 'list_tools':
        console.log(JSON.stringify(tools));
        break;
        
      case 'invoke':
        if (!message.tool) {
          console.log(JSON.stringify({ error: 'Tool name is required' }));
          break;
        }
        
        if (message.tool === 'echo') {
          console.log(JSON.stringify({ 
            result: message.args?.message || 'No message provided'
          }));
        } else if (message.tool === 'add') {
          const a = Number(message.args?.a || 0);
          const b = Number(message.args?.b || 0);
          console.log(JSON.stringify({ 
            result: a + b
          }));
        } else {
          console.log(JSON.stringify({ 
            error: `Unknown tool: ${message.tool}`
          }));
        }
        break;
        
      default:
        console.log(JSON.stringify({ 
          error: `Unknown message type: ${message.type}`
        }));
    }
  } catch (err) {
    console.log(JSON.stringify({ 
      error: `Error processing message: ${err.message}`
    }));
  }
});

// Keep the process running
process.on('SIGINT', () => {
  rl.close();
  process.exit(0);
});
