import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:11434/v1',
  apiKey: 'dummy',
});

async function testOllamaTools() {
  try {
    console.log('Testing Ollama function calling support...\n');
    
    const tools = [
      {
        type: 'function',
        function: {
          name: 'get_weather',
          description: 'Get the current weather in a given location',
          parameters: {
            type: 'object',
            properties: {
              location: {
                type: 'string',
                description: 'The city and state, e.g. San Francisco, CA',
              },
              unit: {
                type: 'string',
                enum: ['celsius', 'fahrenheit'],
              },
            },
            required: ['location'],
          },
        },
      },
    ];

    const messages = [
      {
        role: 'user',
        content: 'What is the weather like in Boston?',
      },
    ];

    console.log('Sending request with tools...');
    const completion = await client.chat.completions.create({
      model: 'qwen2.5-coder:32b-128k',
      messages: messages,
      tools: tools,
      tool_choice: 'auto',
    });

    console.log('Response:', JSON.stringify(completion, null, 2));
    
    if (completion.choices[0].message.tool_calls) {
      console.log('\n✅ Ollama supports function calling!');
      console.log('Tool calls:', completion.choices[0].message.tool_calls);
    } else {
      console.log('\n❌ No tool calls in response. Content:', completion.choices[0].message.content);
    }
  } catch (error) {
    console.error('\n❌ Error:', error.message);
    if (error.response) {
      console.error('Response data:', error.response.data);
    }
  }
}

testOllamaTools();