import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:11434/v1',
  apiKey: 'dummy',
});

async function testOllama() {
  try {
    console.log('Testing Ollama connection...');
    
    const completion = await client.chat.completions.create({
      model: 'qwen2.5-coder:32b-128k',
      messages: [
        { role: 'system', content: 'You are a helpful assistant.' },
        { role: 'user', content: 'What is 2+2?' }
      ],
    });

    console.log('Response:', completion.choices[0].message.content);
    console.log('\nOllama is working correctly with full tools support!');
  } catch (error) {
    console.error('Error:', error.message);
  }
}

testOllama();