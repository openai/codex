import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:11434/v1',
  apiKey: 'dummy',
});

async function testOllamaWithCodexFix() {
  try {
    console.log('Testing Ollama with Codex fix for function calling...\n');
    
    const messages = [
      {
        role: 'system',
        content: 'You are a helpful assistant that can run shell commands when needed.',
      },
      {
        role: 'user',
        content: 'List the files in the current directory',
      },
    ];

    const tools = [
      {
        type: 'function',
        function: {
          name: 'shell',
          description: 'Runs a shell command and returns its output',
          parameters: {
            type: 'object',
            properties: {
              command: {
                type: 'array',
                items: { type: 'string' },
                description: 'The command to execute as an array of strings',
              },
              workdir: {
                type: 'string',
                description: 'The working directory for the command',
              },
              timeout: {
                type: 'number',
                description: 'The maximum time to wait for the command to complete in milliseconds',
              },
            },
            required: ['command'],
            additionalProperties: false,
          },
        },
      },
    ];

    console.log('Sending request with shell tool...');
    const completion = await client.chat.completions.create({
      model: 'qwen2.5-coder:32b-128k',
      messages: messages,
      tools: tools,
      tool_choice: 'auto',
      stream: true,
    });

    let fullContent = '';
    let functionCall = null;

    for await (const chunk of completion) {
      const choice = chunk.choices[0];
      
      if (choice.delta.content) {
        fullContent += choice.delta.content;
        process.stdout.write(choice.delta.content);
      }
      
      if (choice.delta.tool_calls) {
        console.log('\n\nTool call detected:', JSON.stringify(choice.delta.tool_calls, null, 2));
        functionCall = choice.delta.tool_calls;
      }
    }

    console.log('\n\nFull response content:', fullContent);
    
    // Check if the content looks like a function call (Ollama's format)
    if (fullContent.trim().startsWith('{')) {
      try {
        const parsed = JSON.parse(fullContent);
        if (parsed.name === 'shell' && parsed.arguments) {
          console.log('\n✅ Ollama returned a function call in text format!');
          console.log('Function:', parsed.name);
          console.log('Arguments:', parsed.arguments);
          console.log('\nThe Codex fix should transform this into a proper tool call.');
        }
      } catch (e) {
        console.log('\n❌ Could not parse function call from content');
      }
    } else if (functionCall) {
      console.log('\n✅ Proper tool call format detected!');
    } else {
      console.log('\n❌ No function call detected');
    }
    
  } catch (error) {
    console.error('\n❌ Error:', error.message);
  }
}

testOllamaWithCodexFix();