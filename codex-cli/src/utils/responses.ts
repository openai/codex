import type { OpenAI } from 'openai';
import type { ResponseCreateParams, Response } from 'openai/resources/responses/responses';
// Define interfaces based on OpenAI API documentation
type ResponseCreateInput = ResponseCreateParams;
type ResponseOutput = Response;


type ResponseEvent =
  | { type: 'response.created'; response: Partial<ResponseOutput> }
  | { type: 'response.in_progress'; response: Partial<ResponseOutput> }
  | { type: 'response.output_item.added'; output_index: number; item: any }
  | { type: 'response.content_part.added'; item_id: string; output_index: number; content_index: number; part: any }
  | { type: 'response.output_text.delta'; item_id: string; output_index: number; content_index: number; delta: string }
  | { type: 'response.output_text.done'; item_id: string; output_index: number; content_index: number; text: string }
  | { type: 'response.function_call_arguments.delta'; item_id: string; output_index: number; content_index: number; delta: string }
  | { type: 'response.function_call_arguments.done'; item_id: string; output_index: number; content_index: number; arguments: string }
  | { type: 'response.content_part.done'; item_id: string; output_index: number; content_index: number; part: any }
  | { type: 'response.output_item.done'; output_index: number; item: any }
  | { type: 'response.completed'; response: ResponseOutput }
  | { type: 'error'; code: string; message: string; param: string | null };

// Global map to store conversation histories
const conversationHistories = new Map<
  string,
  {
    previous_response_id: string | null;
    messages: OpenAI.Chat.Completions.ChatCompletionMessageParam[];
  }
>();

// Utility function to generate unique IDs
function generateId(prefix: string = 'msg'): string {
  return `${prefix}_${Math.random().toString(36).substr(2, 9)}`;
}

// Function to convert ResponseInputItem to ChatCompletionMessageParam
type ResponseInputItem = ResponseCreateInput['input'][number];

function convertInputItemToMessage(
  item: ResponseInputItem
): OpenAI.Chat.Completions.ChatCompletionMessageParam {
  if (item.type === 'message') {
    const content = item.content
      .filter((c) => c.type === 'input_text')
      .map((c) => c.text)
      .join('');
    return { role: item.role, content };
  } else if (item.type === 'function_call_output') {
    return {
      role: 'tool',
      tool_call_id: item.call_id,
      content: item.output,
    };
  }
  throw new Error(`Unsupported input item type: ${item.type}`);
}

// Function to get full messages including history
function getFullMessages(input: ResponseCreateInput): OpenAI.Chat.Completions.ChatCompletionMessageParam[] {
  let baseHistory: OpenAI.Chat.Completions.ChatCompletionMessageParam[] = [];
  if (input.previous_response_id) {
    const prev = conversationHistories.get(input.previous_response_id);
    if (!prev) throw new Error(`Previous response not found: ${input.previous_response_id}`);
    baseHistory = prev.messages;
  }
  const newInputMessages = input.input.map(convertInputItemToMessage);
  const messages = [...baseHistory, ...newInputMessages];
  if(input.instructions && messages[0]?.role !== "system" && messages[0]?.role !== "developer") {
    return [{role: 'system', content: input.instructions}, ...messages];
  }
  return messages;
}

// Function to convert tools
function convertTools(tools?: ResponseCreateInput['tools']): OpenAI.Chat.Completions.ChatCompletionTool[] | undefined {
  return tools
    ?.filter((tool) => tool.type === 'function')
    .map((tool) => ({
      type: 'function',
      function: {
        name: tool.name,
        description: tool.description,
        parameters: tool.parameters,
      },
    }));
}

// Main function with overloading
async function responsesCreateViaChatCompletions(openai: OpenAI, input: ResponseCreateInput & { stream: true }): Promise<AsyncGenerator<ResponseEvent>>;
async function responsesCreateViaChatCompletions(openai: OpenAI, input: ResponseCreateInput & { stream?: false }): Promise<ResponseOutput>;
async function responsesCreateViaChatCompletions(openai: OpenAI, input: ResponseCreateInput): Promise<ResponseOutput | AsyncGenerator<ResponseEvent>> {
  if (input.stream) {
    return streamResponses(openai, input);
  } else {
    return nonStreamResponses(openai, input);
  }
}

// Non-streaming implementation
async function nonStreamResponses(openai: OpenAI, input: ResponseCreateInput): Promise<ResponseOutput> {
  const fullMessages = getFullMessages(input);
  const chatTools = convertTools(input.tools);
  const webSearchOptions = input.tools?.some((tool) => tool.type === 'builtin' && tool.name === 'web_search') ? {} : undefined;

  const chatInput: OpenAI.Chat.Completions.ChatCompletionCreateParams = {
    model: input.model,
    messages: fullMessages,
    tools: chatTools,
    web_search_options: webSearchOptions,
    temperature: input.temperature,
    top_p: input.top_p,
    stream: false,
    store: input.store,
    user: input.user,
    metadata: input.metadata,
  };


  try {
    const chatResponse = await openai.chat.completions.create(chatInput);
    if (!('choices' in chatResponse) || chatResponse.choices.length === 0) {
      throw new Error('No choices in chat completion response');
    }
    const assistantMessage = chatResponse.choices?.[0]?.message;
    if (!assistantMessage) {
      throw new Error('No assistant message in chat completion response');
    }

    // Construct ResponseOutput
    const responseId = generateId('resp');
    const outputItemId = generateId('msg');
    const outputContent: Array<any> = [];
    if (assistantMessage.tool_calls) {
      outputContent.push(
        ...assistantMessage.tool_calls.map((toolCall) => ({
          type: 'function_call',
          call_id: toolCall.id,
          name: toolCall.function.name,
          arguments: toolCall.function.arguments,
        }))
      );
    }
    if (assistantMessage.content) {
      outputContent.push({
        type: 'output_text',
        text: assistantMessage.content,
        annotations: [],
      });
    }
    const responseOutput: ResponseOutput = {
      id: responseId,
      object: 'response',
      created_at: Math.floor(Date.now() / 1000),
      status: 'completed',
      error: null,
      incomplete_details: null,
      instructions: null,
      max_output_tokens: null,
      model: chatResponse.model,
      output: [
        {
          type: 'message',
          id: outputItemId,
          status: 'completed',
          role: 'assistant',
          content: outputContent,
        },
      ],
      parallel_tool_calls: input.parallel_tool_calls ?? false,
      previous_response_id: input.previous_response_id ?? null,
      reasoning: { effort: null, summary: null },
      store: input.store ?? false,
      temperature: input.temperature ?? 1.0,
      text: { format: { type: 'text' } },
      tool_choice: input.tool_choice ?? 'auto',
      tools: input.tools ?? [],
      top_p: input.top_p ?? 1.0,
      truncation: input.truncation ?? 'disabled',
      usage: chatResponse.usage ? {
        input_tokens: chatResponse.usage.prompt_tokens,
        input_tokens_details: { cached_tokens: 0 },
        output_tokens: chatResponse.usage.completion_tokens,
        output_tokens_details: { reasoning_tokens: 0 },
        total_tokens: chatResponse.usage.total_tokens,
      } : null,
      user: input.user ?? null,
      metadata: input.metadata ?? {},
    };

    // Store history
    const newHistory = [...fullMessages, assistantMessage];
    conversationHistories.set(responseId, {
      previous_response_id: input.previous_response_id ?? null,
      messages: newHistory,
    });

    return responseOutput;
  } catch (error: any) {
    throw new Error(`Failed to process chat completion: ${error.message}`);
  }
}

// Streaming implementation
async function* streamResponses(openai: OpenAI, input: ResponseCreateInput): AsyncGenerator<ResponseEvent> {
  const fullMessages = getFullMessages(input);
  const chatTools = convertTools(input.tools);
  const webSearchOptions = input.tools?.some((tool) => tool.type === 'builtin' && tool.name === 'web_search') ? {} : undefined;

  const chatInput: OpenAI.Chat.Completions.ChatCompletionCreateParams = {
    model: input.model,
    messages: fullMessages,
    tools: chatTools,
    web_search_options: webSearchOptions,
    temperature: input.temperature ?? 1.0,
    top_p: input.top_p ?? 1.0,
    tool_choice: input.tool_choice ?? 'auto',
    stream: true,
    store: input.store ?? false,
    user: input.user,
    metadata: input.metadata,
  };

  try {
    const stream = await openai.chat.completions.create(chatInput);

    // Initialize state
    const responseId = generateId('resp');
    const outputItemId = generateId('msg');
    let textContentAdded = false;
    let textContent = '';
    const toolCalls = new Map<number, { id: string; name: string; arguments: string }>();
    let usage: any = null;
    let finalOutputItem: any = [];
    // Initial response
    const initialResponse: Partial<ResponseOutput> = {
      id: responseId,
      object: 'response',
      created_at: Math.floor(Date.now() / 1000),
      status: 'in_progress',
      model: input.model,
      output: [],
      error: null,
      incomplete_details: null,
      instructions: null,
      max_output_tokens: null,
      parallel_tool_calls: true,
      previous_response_id: input.previous_response_id ?? null,
      reasoning: { effort: null, summary: null },
      store: input.store ?? false,
      temperature: input.temperature ?? 1.0,
      text: { format: { type: 'text' } },
      tool_choice: input.tool_choice ?? 'auto',
      tools: input.tools ?? [],
      top_p: input.top_p ?? 1.0,
      truncation: input.truncation ?? 'disabled',
      usage: null,
      user: input.user ?? null,
      metadata: input.metadata ?? {},
    };
    yield { type: 'response.created', response: initialResponse };
    yield { type: 'response.in_progress', response: initialResponse };
    let isToolCall = false;
    for await (const chunk of stream) {
      // console.error('\nCHUNK: ', JSON.stringify(chunk));
      const choice = chunk.choices[0];
      if (!choice) continue;
      if (!isToolCall && (('tool_calls' in choice.delta && choice.delta.tool_calls) || choice.finish_reason === 'tool_calls')) {
        isToolCall = true;
      }

      usage = chunk.usage;
      if (isToolCall) {
        for (const tcDelta of choice.delta.tool_calls || []) {
          const tcIndex = tcDelta.index;
          const content_index = textContentAdded ? tcIndex + 1 : tcIndex;

          if (!toolCalls.has(tcIndex)) {
            // New tool call
            const toolCallId = tcDelta.id || generateId('call');
            const functionName = tcDelta.function?.name || '';

            yield {
              type: 'response.output_item.added',
              item: {
                type: 'function_call',
                id: outputItemId,
                status: 'in_progress',
                call_id: toolCallId,
                name: functionName,
                arguments: '',
              },
              output_index: 0,
            };
            toolCalls.set(tcIndex, { id: toolCallId, name: functionName, arguments: '' });
          }

          if (tcDelta.function?.arguments) {
            const current = toolCalls.get(tcIndex);
            if (current) {
              current.arguments += tcDelta.function.arguments;
              yield {
                type: 'response.function_call_arguments.delta',
                item_id: outputItemId,
                output_index: 0,
                content_index,
                delta: tcDelta.function.arguments,
              };
            }
          }
        }

        if(choice.finish_reason === 'tool_calls') {
          for (const [tcIndex, tc] of toolCalls) {
            const item = {
              type: 'function_call',
              id: outputItemId,
              status: 'completed',
              call_id: tc.id,
              name: tc.name,
              arguments: tc.arguments,
            };
            yield {
              type: 'response.function_call_arguments.done',
              item,
              output_index: tcIndex,
            };
            yield {
              type: 'response.output_item.done',
              output_index: tcIndex,
              item,
            };
            finalOutputItem.push(item);
          }
        } else {
          continue;
        }

      } else {
        if(!textContentAdded) {
          yield {
            type: 'response.content_part.added',
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            part: { type: 'output_text', text: '', annotations: [] },
          };
          textContentAdded = true;
        } else if (choice.delta.content) {
          yield {
            type: 'response.output_text.delta',
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            delta: choice.delta.content,
          };
          textContent += choice.delta.content;
        }
        if(choice.finish_reason) {
          yield {
            type: 'response.output_text.done',
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            text: textContent,
          };
          yield {
            type: 'response.content_part.done',
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            part: { type: 'output_text', text: textContent, annotations: [] },
          };
          const item = {
            type: 'message',
            id: outputItemId,
            status: 'completed',
            role: 'assistant',
            content: [{ type: 'output_text', text: textContent, annotations: [] }],
          };
          yield {
            type: 'response.output_item.done',
            output_index: 0,
            item,
          };
          finalOutputItem.push(item);
        } else {
          continue;
        }
      }
      

      // Construct final response
      const finalResponse: ResponseOutput = {
        id: responseId,
        object: 'response',
        created_at: initialResponse.created_at,
        status: 'completed',
        error: null,
        incomplete_details: null,
        instructions: null,
        max_output_tokens: null,
        model: chunk.model || input.model,
        output: finalOutputItem,
        parallel_tool_calls: true,
        previous_response_id: input.previous_response_id ?? null,
        reasoning: { effort: null, summary: null },
        store: input.store ?? false,
        temperature: input.temperature ?? 1.0,
        text: { format: { type: 'text' } },
        tool_choice: input.tool_choice ?? 'auto',
        tools: input.tools ?? [],
        top_p: input.top_p ?? 1.0,
        truncation: input.truncation ?? 'disabled',
        usage,
        user: input.user ?? null,
        metadata: input.metadata ?? {},
      };

      // Store history
      const assistantMessage = {
        role: 'assistant' as const,
        content: textContent || null,
      };
      if(toolCalls.size > 0) {
        assistantMessage.tool_calls = Array.from(toolCalls.values().map((tc) => ({
          id: tc.id,
          type: 'function' as const,
          function: { name: tc.name, arguments: tc.arguments },
        })));
      }
      const newHistory = [...fullMessages, assistantMessage];
      conversationHistories.set(responseId, {
        previous_response_id: input.previous_response_id ?? null,
        messages: newHistory,
      });

      yield { type: 'response.completed', response: finalResponse };
    }
  } catch (error: any) {
    yield { type: 'error', code: error.code || 'unknown', message: error.message, param: null };
  }
}

export { responsesCreateViaChatCompletions, ResponseCreateInput, ResponseOutput, ResponseEvent };
