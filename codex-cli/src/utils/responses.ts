import type { OpenAI } from "openai";
import type {
  ResponseCreateParams,
  Response,
} from "openai/resources/responses/responses";
import { repairOllamaFunctionCall } from "./repair-json.js";

// Define interfaces based on OpenAI API documentation
type ResponseCreateInput = ResponseCreateParams;
type ResponseOutput = Response;
// interface ResponseOutput {
//   id: string;
//   object: 'response';
//   created_at: number;
//   status: 'completed' | 'failed' | 'in_progress' | 'incomplete';
//   error: { code: string; message: string } | null;
//   incomplete_details: { reason: string } | null;
//   instructions: string | null;
//   max_output_tokens: number | null;
//   model: string;
//   output: Array<{
//     type: 'message';
//     id: string;
//     status: 'completed' | 'in_progress';
//     role: 'assistant';
//     content: Array<{
//       type: 'output_text' | 'function_call';
//       text?: string;
//       annotations?: Array<any>;
//       tool_call?: {
//         id: string;
//         type: 'function';
//         function: { name: string; arguments: string };
//       };
//     }>;
//   }>;
//   parallel_tool_calls: boolean;
//   previous_response_id: string | null;
//   reasoning: { effort: string | null; summary: string | null };
//   store: boolean;
//   temperature: number;
//   text: { format: { type: 'text' } };
//   tool_choice: string | object;
//   tools: Array<any>;
//   top_p: number;
//   truncation: string;
//   usage: {
//     input_tokens: number;
//     input_tokens_details: { cached_tokens: number };
//     output_tokens: number;
//     output_tokens_details: { reasoning_tokens: number };
//     total_tokens: number;
//   } | null;
//   user: string | null;
//   metadata: Record<string, string>;
// }

// Define types for the ResponseItem content and parts
type ResponseContentPart = {
  type: string;
  [key: string]: unknown;
};

type ResponseItemType = {
  type: string;
  id?: string;
  status?: string;
  role?: string;
  content?: Array<ResponseContentPart>;
  [key: string]: unknown;
};

type ResponseEvent =
  | { type: "response.created"; response: Partial<ResponseOutput> }
  | { type: "response.in_progress"; response: Partial<ResponseOutput> }
  | {
      type: "response.output_item.added";
      output_index: number;
      item: ResponseItemType;
    }
  | {
      type: "response.content_part.added";
      item_id: string;
      output_index: number;
      content_index: number;
      part: ResponseContentPart;
    }
  | {
      type: "response.output_text.delta";
      item_id: string;
      output_index: number;
      content_index: number;
      delta: string;
    }
  | {
      type: "response.output_text.done";
      item_id: string;
      output_index: number;
      content_index: number;
      text: string;
    }
  | {
      type: "response.function_call_arguments.delta";
      item_id: string;
      output_index: number;
      content_index: number;
      delta: string;
    }
  | {
      type: "response.function_call_arguments.done";
      item_id: string;
      output_index: number;
      content_index: number;
      arguments: string;
    }
  | {
      type: "response.content_part.done";
      item_id: string;
      output_index: number;
      content_index: number;
      part: ResponseContentPart;
    }
  | {
      type: "response.output_item.done";
      output_index: number;
      item: ResponseItemType;
    }
  | { type: "response.completed"; response: ResponseOutput }
  | { type: "error"; code: string; message: string; param: string | null };

// Define a type for tool call data
type ToolCallData = {
  id: string;
  name: string;
  arguments: string;
};

// Define a type for usage data
type UsageData = {
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
  input_tokens?: number;
  input_tokens_details?: { cached_tokens: number };
  output_tokens?: number;
  output_tokens_details?: { reasoning_tokens: number };
  [key: string]: unknown;
};

// Define a type for content output
type ResponseContentOutput =
  | {
      type: "function_call";
      call_id: string;
      name: string;
      arguments: string;
      [key: string]: unknown;
    }
  | {
      type: "output_text";
      text: string;
      annotations: Array<unknown>;
      [key: string]: unknown;
    };

// Global map to store conversation histories
const conversationHistories = new Map<
  string,
  {
    previous_response_id: string | null;
    messages: Array<OpenAI.Chat.Completions.ChatCompletionMessageParam>;
  }
>();

// Utility function to generate unique IDs
function generateId(prefix: string = "msg"): string {
  return `${prefix}_${Math.random().toString(36).substr(2, 9)}`;
}

// Function to convert ResponseInputItem to ChatCompletionMessageParam
type ResponseInputItem = ResponseCreateInput["input"][number];

function convertInputItemToMessage(
  item: string | ResponseInputItem,
): OpenAI.Chat.Completions.ChatCompletionMessageParam {
  // Handle string inputs as content for a user message
  if (typeof item === "string") {
    return { role: "user", content: item };
  }

  // At this point we know it's a ResponseInputItem
  const responseItem = item;

  if (responseItem.type === "message") {
    // Use a more specific type assertion for the message content
    const content = Array.isArray(responseItem.content)
      ? responseItem.content
          .filter((c) => typeof c === "object" && c.type === "input_text")
          .map((c) =>
            typeof c === "object" && "text" in c
              ? (c["text"] as string) || ""
              : "",
          )
          .join("")
      : "";
    return { role: responseItem.role, content };
  } else if (responseItem.type === "function_call_output") {
    return {
      role: "tool",
      tool_call_id: responseItem.call_id,
      content: responseItem.output,
    };
  }
  throw new Error(`Unsupported input item type: ${responseItem.type}`);
}

// Function to get full messages including history
function getFullMessages(
  input: ResponseCreateInput,
): Array<OpenAI.Chat.Completions.ChatCompletionMessageParam> {
  let baseHistory: Array<OpenAI.Chat.Completions.ChatCompletionMessageParam> =
    [];
  if (input.previous_response_id) {
    const prev = conversationHistories.get(input.previous_response_id);
    if (!prev) {
      throw new Error(
        `Previous response not found: ${input.previous_response_id}`,
      );
    }
    baseHistory = prev.messages;
  }

  // Handle both string and ResponseInputItem in input.input
  const newInputMessages = Array.isArray(input.input)
    ? input.input.map(convertInputItemToMessage)
    : [convertInputItemToMessage(input.input)];

  const messages = [...baseHistory, ...newInputMessages];
  if (
    input.instructions &&
    messages[0]?.role !== "system" &&
    messages[0]?.role !== "developer"
  ) {
    return [{ role: "system", content: input.instructions }, ...messages];
  }
  return messages;
}

// Function to convert tools
function convertTools(
  tools?: ResponseCreateInput["tools"],
): Array<OpenAI.Chat.Completions.ChatCompletionTool> | undefined {
  return tools
    ?.filter((tool) => tool.type === "function")
    .map((tool) => ({
      type: "function" as const,
      function: {
        name: tool.name,
        description: tool.description || undefined,
        parameters: tool.parameters,
      },
    }));
}

const createCompletion = (openai: OpenAI, input: ResponseCreateInput) => {
  const fullMessages = getFullMessages(input);
  const chatTools = convertTools(input.tools);
  const webSearchOptions = input.tools?.some(
    (tool) => tool.type === "function" && tool.name === "web_search",
  )
    ? {}
    : undefined;

  const chatInput: OpenAI.Chat.Completions.ChatCompletionCreateParams = {
    model: input.model,
    messages: fullMessages,
    tools: chatTools,
    web_search_options: webSearchOptions,
    temperature: input.temperature,
    top_p: input.top_p,
    tool_choice: (input.tool_choice === "auto"
      ? "auto"
      : input.tool_choice) as OpenAI.Chat.Completions.ChatCompletionCreateParams["tool_choice"],
    stream: input.stream || false,
    user: input.user,
    metadata: input.metadata,
  };

  return openai.chat.completions.create(chatInput);
};

// Main function with overloading
async function responsesCreateViaChatCompletions(
  openai: OpenAI,
  input: ResponseCreateInput & { stream: true },
): Promise<AsyncGenerator<ResponseEvent>>;
async function responsesCreateViaChatCompletions(
  openai: OpenAI,
  input: ResponseCreateInput & { stream?: false },
): Promise<ResponseOutput>;
async function responsesCreateViaChatCompletions(
  openai: OpenAI,
  input: ResponseCreateInput,
): Promise<ResponseOutput | AsyncGenerator<ResponseEvent>> {
  const completion = await createCompletion(openai, input);
  if (input.stream) {
    return streamResponses(
      input,
      completion as AsyncIterable<OpenAI.ChatCompletionChunk>,
      openai,
    );
  } else {
    return nonStreamResponses(
      input,
      completion as unknown as OpenAI.Chat.Completions.ChatCompletion,
    );
  }
}

// Non-streaming implementation
async function nonStreamResponses(
  input: ResponseCreateInput,
  completion: OpenAI.Chat.Completions.ChatCompletion,
): Promise<ResponseOutput> {
  const fullMessages = getFullMessages(input);

  try {
    const chatResponse = completion;
    if (!("choices" in chatResponse) || chatResponse.choices.length === 0) {
      throw new Error("No choices in chat completion response");
    }
    const assistantMessage = chatResponse.choices?.[0]?.message;
    if (!assistantMessage) {
      throw new Error("No assistant message in chat completion response");
    }

    // Construct ResponseOutput
    const responseId = generateId("resp");
    const outputItemId = generateId("msg");
    const outputContent: Array<ResponseContentOutput> = [];

    // Check if the response contains tool calls
    const hasFunctionCalls =
      assistantMessage.tool_calls && assistantMessage.tool_calls.length > 0;

    if (hasFunctionCalls && assistantMessage.tool_calls) {
      for (const toolCall of assistantMessage.tool_calls) {
        if (toolCall.type === "function") {
          outputContent.push({
            type: "function_call",
            call_id: toolCall.id,
            name: toolCall.function.name,
            arguments: toolCall.function.arguments,
          });
        }
      }
    }

    if (assistantMessage.content) {
      outputContent.push({
        type: "output_text",
        text: assistantMessage.content,
        annotations: [],
      });
    }

    // Create response with appropriate status and properties
    const responseOutput = {
      id: responseId,
      object: "response",
      created_at: Math.floor(Date.now() / 1000),
      status: hasFunctionCalls ? "requires_action" : "completed",
      error: null,
      incomplete_details: null,
      instructions: null,
      max_output_tokens: null,
      model: chatResponse.model,
      output: [
        {
          type: "message",
          id: outputItemId,
          status: "completed",
          role: "assistant",
          content: outputContent,
        },
      ],
      parallel_tool_calls: input.parallel_tool_calls ?? false,
      previous_response_id: input.previous_response_id ?? null,
      reasoning: null,
      temperature: input.temperature,
      text: { format: { type: "text" } },
      tool_choice: input.tool_choice ?? "auto",
      tools: input.tools ?? [],
      top_p: input.top_p,
      truncation: input.truncation ?? "disabled",
      usage: chatResponse.usage
        ? {
            input_tokens: chatResponse.usage.prompt_tokens,
            input_tokens_details: { cached_tokens: 0 },
            output_tokens: chatResponse.usage.completion_tokens,
            output_tokens_details: { reasoning_tokens: 0 },
            total_tokens: chatResponse.usage.total_tokens,
          }
        : undefined,
      user: input.user ?? undefined,
      metadata: input.metadata ?? {},
      output_text: "",
    } as ResponseOutput;

    // Add required_action property for tool calls
    if (hasFunctionCalls && assistantMessage.tool_calls) {
      // Define type with required action
      type ResponseWithAction = Partial<ResponseOutput> & {
        required_action: unknown;
      };

      // Use the defined type for the assertion
      (responseOutput as ResponseWithAction).required_action = {
        type: "submit_tool_outputs",
        submit_tool_outputs: {
          tool_calls: assistantMessage.tool_calls.map((toolCall) => ({
            id: toolCall.id,
            type: toolCall.type,
            function: {
              name: toolCall.function.name,
              arguments: toolCall.function.arguments,
            },
          })),
        },
      };
    }

    // Store history
    const newHistory = [...fullMessages, assistantMessage];
    conversationHistories.set(responseId, {
      previous_response_id: input.previous_response_id ?? null,
      messages: newHistory,
    });

    return responseOutput;
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to process chat completion: ${errorMessage}`);
  }
}

// Streaming implementation
async function* streamResponses(
  input: ResponseCreateInput,
  completion: AsyncIterable<OpenAI.ChatCompletionChunk>,
  openai: OpenAI,
): AsyncGenerator<ResponseEvent> {
  const fullMessages = getFullMessages(input);

  const responseId = generateId("resp");
  const outputItemId = generateId("msg");
  let textContentAdded = false;
  let textContent = "";
  const toolCalls = new Map<number, ToolCallData>();
  let usage: UsageData | null = null;
  const finalOutputItem: Array<ResponseContentOutput> = [];
  
  // Detect if this is Ollama based on the OpenAI client's baseURL
  const isOllama = openai.baseURL?.includes('11434') || 
                   openai.baseURL?.includes('localhost:11434') ||
                   openai.baseURL?.toLowerCase().includes('ollama') ||
                   false;
  
  if (process.env.DEBUG || process.env.CODEX_DEBUG) {
    console.error('[streamResponses] baseURL:', openai.baseURL, 'isOllama:', isOllama);
  }
  
  // Initial response
  const initialResponse: Partial<ResponseOutput> = {
    id: responseId,
    object: "response" as const,
    created_at: Math.floor(Date.now() / 1000),
    status: "in_progress" as const,
    model: input.model,
    output: [],
    error: null,
    incomplete_details: null,
    instructions: null,
    max_output_tokens: null,
    parallel_tool_calls: true,
    previous_response_id: input.previous_response_id ?? null,
    reasoning: null,
    temperature: input.temperature,
    text: { format: { type: "text" } },
    tool_choice: input.tool_choice ?? "auto",
    tools: input.tools ?? [],
    top_p: input.top_p,
    truncation: input.truncation ?? "disabled",
    usage: undefined,
    user: input.user ?? undefined,
    metadata: input.metadata ?? {},
    output_text: "",
  };
  yield { type: "response.created", response: initialResponse };
  yield { type: "response.in_progress", response: initialResponse };
  let isToolCall = false;
  for await (const chunk of completion as AsyncIterable<OpenAI.ChatCompletionChunk>) {
    // console.error('\nCHUNK: ', JSON.stringify(chunk));
    const choice = chunk.choices?.[0];
    if (!choice) {
      continue;
    }
    if (
      !isToolCall &&
      (("tool_calls" in choice.delta && choice.delta.tool_calls) ||
        choice.finish_reason === "tool_calls")
    ) {
      isToolCall = true;
    }

    if (chunk.usage) {
      usage = {
        prompt_tokens: chunk.usage.prompt_tokens,
        completion_tokens: chunk.usage.completion_tokens,
        total_tokens: chunk.usage.total_tokens,
        input_tokens: chunk.usage.prompt_tokens,
        input_tokens_details: { cached_tokens: 0 },
        output_tokens: chunk.usage.completion_tokens,
        output_tokens_details: { reasoning_tokens: 0 },
      };
    }
    if (isToolCall) {
      for (const tcDelta of choice.delta.tool_calls || []) {
        const tcIndex = tcDelta.index;
        const content_index = textContentAdded ? tcIndex + 1 : tcIndex;

        if (!toolCalls.has(tcIndex)) {
          // New tool call
          const toolCallId = tcDelta.id || generateId("call");
          const functionName = tcDelta.function?.name || "";

          yield {
            type: "response.output_item.added",
            item: {
              type: "function_call",
              id: outputItemId,
              status: "in_progress",
              call_id: toolCallId,
              name: functionName,
              arguments: "",
            },
            output_index: 0,
          };
          toolCalls.set(tcIndex, {
            id: toolCallId,
            name: functionName,
            arguments: "",
          });
        }

        if (tcDelta.function?.arguments) {
          const current = toolCalls.get(tcIndex);
          if (current) {
            current.arguments += tcDelta.function.arguments;
            yield {
              type: "response.function_call_arguments.delta",
              item_id: outputItemId,
              output_index: 0,
              content_index,
              delta: tcDelta.function.arguments,
            };
          }
        }
      }

      if (choice.finish_reason === "tool_calls") {
        for (const [tcIndex, tc] of toolCalls) {
          const item = {
            type: "function_call",
            id: outputItemId,
            status: "completed",
            call_id: tc.id,
            name: tc.name,
            arguments: tc.arguments,
          };
          yield {
            type: "response.function_call_arguments.done",
            item_id: outputItemId,
            output_index: tcIndex,
            content_index: textContentAdded ? tcIndex + 1 : tcIndex,
            arguments: tc.arguments,
          };
          yield {
            type: "response.output_item.done",
            output_index: tcIndex,
            item,
          };
          finalOutputItem.push(item as unknown as ResponseContentOutput);
        }
      } else {
        continue;
      }
    } else {
      if (choice.delta.content?.length) {
        // Special handling for Ollama - accumulate content but don't emit yet
        if (isOllama) {
          textContent += choice.delta.content;
          // Don't emit anything during streaming for Ollama
          continue;
        } else {
          // Non-Ollama providers - normal streaming
          textContent += choice.delta.content;
          // Non-Ollama providers - normal streaming
          if (!textContentAdded) {
            yield {
              type: "response.content_part.added",
              item_id: outputItemId,
              output_index: 0,
              content_index: 0,
              part: { type: "output_text", text: "", annotations: [] },
            };
            textContentAdded = true;
          }
          
          yield {
            type: "response.output_text.delta",
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            delta: choice.delta.content,
          };
        }
      }
      if (choice.finish_reason) {
        // Check if this looks like an Ollama function call
        let isOllamaFunctionCall = false;
        let functionName = "";
        let functionArgs = "";
        
        // For Ollama, process the entire content and filter out JSON function calls
        if (isOllama && textContent) {
          const functionCalls: Array<{name: string, args: string}> = [];
          let processedContent = textContent;
          let foundJson = true;
          
          // Keep looking for JSON function calls until we can't find any more
          while (foundJson) {
            foundJson = false;
            
            // Look for the start of a JSON function call
            const startMatch = processedContent.match(/\{\s*"name"\s*:\s*"[^"]+"\s*,\s*"arguments"\s*:/);
            
            if (startMatch && startMatch.index !== undefined) {
              const startIdx = startMatch.index;
              let braceCount = 0;
              let inString = false;
              let escapeNext = false;
              let endIdx = -1;
              
              // Parse from the start to find the matching closing brace
              for (let i = startIdx; i < processedContent.length; i++) {
                const char = processedContent[i];
                
                if (escapeNext) {
                  escapeNext = false;
                  continue;
                }
                
                if (char === '\\') {
                  escapeNext = true;
                  continue;
                }
                
                if (char === '"' && !escapeNext) {
                  inString = !inString;
                  continue;
                }
                
                if (!inString) {
                  if (char === '{') {
                    braceCount++;
                  } else if (char === '}') {
                    braceCount--;
                    if (braceCount === 0) {
                      endIdx = i;
                      break;
                    }
                  }
                }
              }
              
              if (endIdx !== -1) {
                const jsonStr = processedContent.substring(startIdx, endIdx + 1);
                
                // Try to parse as valid JSON first
                try {
                  const parsed = JSON.parse(jsonStr);
                  if (parsed.name && (parsed.arguments || parsed.parameters)) {
                    // It's a function call, extract it and remove from content
                    functionCalls.push({
                      name: parsed.name,
                      args: typeof parsed.arguments === 'string' ? parsed.arguments : JSON.stringify(parsed.arguments)
                    });
                    processedContent = processedContent.substring(0, startIdx) + processedContent.substring(endIdx + 1);
                    foundJson = true;
                  }
                } catch {
                  // Try to repair the JSON
                  const repaired = repairOllamaFunctionCall(jsonStr);
                  if (repaired) {
                    functionCalls.push({
                      name: repaired.name,
                      args: typeof repaired.arguments === 'string' ? repaired.arguments : JSON.stringify(repaired.arguments)
                    });
                    processedContent = processedContent.substring(0, startIdx) + processedContent.substring(endIdx + 1);
                    foundJson = true;
                  } else if (jsonStr.includes('"name"') && jsonStr.includes('"arguments"')) {
                    // Even if we can't parse it, if it looks like a function call, remove it
                    // Try to extract name manually
                    const nameMatch = jsonStr.match(/"name"\s*:\s*"([^"]+)"/);
                    if (nameMatch) {
                      functionCalls.push({
                        name: nameMatch[1],
                        args: "{}" // Default empty args if we can't parse
                      });
                      processedContent = processedContent.substring(0, startIdx) + processedContent.substring(endIdx + 1);
                      foundJson = true;
                    }
                  }
                }
              }
            }
          }
          
          // Clean up extra whitespace and blank lines
          const cleanedText = processedContent.trim().replace(/\n\s*\n\s*\n/g, '\n\n');
          if (cleanedText) {
            if (!textContentAdded) {
              yield {
                type: "response.content_part.added",
                item_id: outputItemId,
                output_index: 0,
                content_index: 0,
                part: { type: "output_text", text: "", annotations: [] },
              };
              textContentAdded = true;
            }
            
            yield {
              type: "response.output_text.delta",
              item_id: outputItemId,
              output_index: 0,
              content_index: 0,
              delta: cleanedText,
            };
            
            yield {
              type: "response.output_text.done",
              item_id: outputItemId,
              output_index: 0,
              content_index: 0,
              text: cleanedText,
            };
            
            yield {
              type: "response.content_part.done",
              item_id: outputItemId,
              output_index: 0,
              content_index: 0,
              part: { type: "output_text", text: cleanedText, annotations: [] },
            };
          }
          
          // Process function calls (use the last one if multiple)
          if (functionCalls.length > 0) {
            const lastCall = functionCalls[functionCalls.length - 1];
            isOllamaFunctionCall = true;
            functionName = lastCall.name;
            functionArgs = lastCall.args;
            
            if (process.env.DEBUG || process.env.CODEX_DEBUG) {
              console.error('[streamResponses] Filtered out', functionCalls.length, 'Ollama function call(s)');
              console.error('[streamResponses] Using last function call:', functionName, functionArgs);
              console.error('[streamResponses] Clean text:', cleanedText);
            }
          }
        } else if (textContent.trim().startsWith('{')) {
          try {
            const parsed = JSON.parse(textContent);
            if (process.env.DEBUG) {
              console.error('[streamResponses] Checking for Ollama function call, parsed:', JSON.stringify(parsed));
            }
            if (parsed.name && (parsed.arguments || parsed.parameters)) {
              isOllamaFunctionCall = true;
              functionName = parsed.name;
              const argsObj = parsed.arguments || parsed.parameters || {};
              functionArgs = typeof argsObj === 'string' ? argsObj : JSON.stringify(argsObj);
              if (process.env.DEBUG) {
                console.error('[streamResponses] Detected Ollama function call:', functionName, 'args:', functionArgs);
              }
            }
          } catch (e) {
            // Try to repair the JSON
            if (process.env.DEBUG) {
              console.error('[streamResponses] Failed to parse as JSON, attempting repair:', e.message);
            }
            const repaired = repairOllamaFunctionCall(textContent.trim());
            if (repaired) {
              isOllamaFunctionCall = true;
              functionName = repaired.name;
              functionArgs = typeof repaired.arguments === 'string' ? repaired.arguments : JSON.stringify(repaired.arguments);
              if (process.env.DEBUG) {
                console.error('[streamResponses] Repaired Ollama function call:', functionName, 'args:', functionArgs);
              }
            } else {
              // Not valid JSON even after repair, treat as regular text
              if (process.env.DEBUG) {
                console.error('[streamResponses] Could not repair JSON, treating as regular text');
              }
            }
          }
        }
        
        if (isOllamaFunctionCall) {
          // Handle as function call
          const toolCallId = generateId("call");
          const functionCallItem = {
            type: "function_call" as const,
            id: outputItemId,
            status: "completed" as const,
            call_id: toolCallId,
            name: functionName,
            arguments: functionArgs,
          };
          
          yield {
            type: "response.output_item.added",
            item: functionCallItem,
            output_index: 0,
          };
          
          yield {
            type: "response.function_call_arguments.delta",
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            delta: functionArgs,
          };
          
          yield {
            type: "response.function_call_arguments.done",
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            arguments: functionArgs,
          };
          
          yield {
            type: "response.output_item.done",
            output_index: 0,
            item: functionCallItem,
          };
          
          finalOutputItem.push(functionCallItem as unknown as ResponseContentOutput);
        } else {
          // Handle as regular text message
          yield {
            type: "response.output_text.done",
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            text: textContent,
          };
          yield {
            type: "response.content_part.done",
            item_id: outputItemId,
            output_index: 0,
            content_index: 0,
            part: { type: "output_text", text: textContent, annotations: [] },
          };
          const item = {
            type: "message",
            id: outputItemId,
            status: "completed",
            role: "assistant",
            content: [
              { type: "output_text", text: textContent, annotations: [] },
            ],
          };
          yield {
            type: "response.output_item.done",
            output_index: 0,
            item,
          };
          finalOutputItem.push(item as unknown as ResponseContentOutput);
        }
      } else {
        continue;
      }
    }

    // Construct final response
    const finalResponse: ResponseOutput = {
      id: responseId,
      object: "response" as const,
      created_at: initialResponse.created_at || Math.floor(Date.now() / 1000),
      status: "completed" as const,
      error: null,
      incomplete_details: null,
      instructions: null,
      max_output_tokens: null,
      model: chunk.model || input.model,
      output: finalOutputItem as unknown as ResponseOutput["output"],
      parallel_tool_calls: true,
      previous_response_id: input.previous_response_id ?? null,
      reasoning: null,
      temperature: input.temperature,
      text: { format: { type: "text" } },
      tool_choice: input.tool_choice ?? "auto",
      tools: input.tools ?? [],
      top_p: input.top_p,
      truncation: input.truncation ?? "disabled",
      usage: usage as ResponseOutput["usage"],
      user: input.user ?? undefined,
      metadata: input.metadata ?? {},
      output_text: "",
    } as ResponseOutput;

    // Store history
    const assistantMessage: OpenAI.Chat.Completions.ChatCompletionMessageParam =
      {
        role: "assistant" as const,
      };

    if (textContent) {
      assistantMessage.content = textContent;
    }

    // Add tool_calls property if needed
    if (toolCalls.size > 0) {
      const toolCallsArray = Array.from(toolCalls.values()).map((tc) => ({
        id: tc.id,
        type: "function" as const,
        function: { name: tc.name, arguments: tc.arguments },
      }));

      // Define a more specific type for the assistant message with tool calls
      type AssistantMessageWithToolCalls =
        OpenAI.Chat.Completions.ChatCompletionMessageParam & {
          tool_calls: Array<{
            id: string;
            type: "function";
            function: {
              name: string;
              arguments: string;
            };
          }>;
        };

      // Use type assertion with the defined type
      (assistantMessage as AssistantMessageWithToolCalls).tool_calls =
        toolCallsArray;
    }
    const newHistory = [...fullMessages, assistantMessage];
    conversationHistories.set(responseId, {
      previous_response_id: input.previous_response_id ?? null,
      messages: newHistory,
    });

    yield { type: "response.completed", response: finalResponse };
  }
}

export {
  responsesCreateViaChatCompletions,
  ResponseCreateInput,
  ResponseOutput,
  ResponseEvent,
};
