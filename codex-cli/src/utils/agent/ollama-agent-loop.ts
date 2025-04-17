/**
 * Ollama implementation of the agent loop.
 * 
 * This module provides an implementation of the agent loop that uses Ollama
 * instead of OpenAI. It's designed to be compatible with the existing AgentLoop
 * class, so it can be used as a drop-in replacement.
 */

import type { ReviewDecision } from "./review.js";
import type { ApplyPatchCommand, ApprovalPolicy } from "../../approvals.js";
import type { AppConfig } from "../config.js";
import type {
  ResponseFunctionToolCall,
  ResponseInputItem,
  ResponseItem,
  ResponseOutputText,
} from "openai/resources/responses/responses.mjs";

import { log, isLoggingEnabled } from "./log.js";
import { parseToolCallArguments } from "../parsers.js";
import {
  ORIGIN,
  CLI_VERSION,
  getSessionId,
  setCurrentModel,
  setSessionId,
} from "../session.js";
import { handleExecCommand } from "./handle-exec-command.js";
import { randomUUID } from "node:crypto";
import {
  convertResponseItemsToOllamaMessages,
  extractOllamaModelName,
  ollamaChatCompletion,
  type OllamaChatCompletionResponse,
  type OllamaMessage,
} from "../ollama-client.js";

export type CommandConfirmation = {
  review: ReviewDecision;
  applyPatch?: ApplyPatchCommand | undefined;
  customDenyMessage?: string;
};

const alreadyProcessedResponses = new Set();

type OllamaAgentLoopParams = {
  model: string;
  config?: AppConfig;
  instructions?: string;
  approvalPolicy: ApprovalPolicy;
  onItem: (item: ResponseItem) => void;
  onLoading: (loading: boolean) => void;

  /** Called when the command is not auto-approved to request explicit user review. */
  getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>;
  onLastResponseId: (lastResponseId: string) => void;
};

/**
 * Implementation of the agent loop that uses Ollama instead of OpenAI.
 * This class is designed to be compatible with the existing AgentLoop class,
 * so it can be used as a drop-in replacement.
 */
export class OllamaAgentLoop {
  private model: string;
  private instructions?: string;
  private approvalPolicy: ApprovalPolicy;
  private config: AppConfig;
  private ollamaApiUrl: string;
  
  // Store conversation history to maintain context between requests
  private conversationHistory: OllamaMessage[] = [];

  private onItem: (item: ResponseItem) => void;
  private onLoading: (loading: boolean) => void;
  private getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>;
  private onLastResponseId: (lastResponseId: string) => void;

  /**
   * A reference to the currently active stream returned from the Ollama API.
   * We keep this so that we can abort the request if the user decides
   * to interrupt the current task (e.g. via the escape hot‑key).
   */
  private currentStream: ReadableStream | null = null;
  /** Incremented with every call to `run()`. Allows us to ignore stray events
   * from streams that belong to a previous run which might still be emitting
   * after the user has canceled and issued a new command. */
  private generation = 0;
  /** AbortController for in‑progress tool calls (e.g. shell commands). */
  private execAbortController: AbortController | null = null;
  /** Set to true when `cancel()` is called so `run()` can exit early. */
  private canceled = false;
  /** Set to true by `terminate()` – prevents any further use of the instance. */
  private terminated = false;
  /** Master abort controller – fires when terminate() is invoked. */
  private readonly hardAbort = new AbortController();

  /**
   * Abort the ongoing request/stream, if any. This allows callers (typically
   * the UI layer) to interrupt the current agent step so the user can issue
   * new instructions without waiting for the model to finish.
   */
  public cancel(): void {
    if (this.terminated) {
      return;
    }

    // Reset the current stream to allow new requests
    this.currentStream = null;
    if (isLoggingEnabled()) {
      log(
        `OllamaAgentLoop.cancel() invoked – currentStream=${Boolean(
          this.currentStream,
        )} execAbortController=${Boolean(
          this.execAbortController,
        )} generation=${this.generation}`,
      );
    }

    this.canceled = true;

    // Abort any in-progress tool calls
    this.execAbortController?.abort();

    // Create a new abort controller for future tool calls
    this.execAbortController = new AbortController();
    if (isLoggingEnabled()) {
      log("OllamaAgentLoop.cancel(): execAbortController.abort() called");
    }

    this.onLoading(false);

    this.generation += 1;
    if (isLoggingEnabled()) {
      log(`OllamaAgentLoop.cancel(): generation bumped to ${this.generation}`);
    }
  }

  /**
   * Hard‑stop the agent loop. After calling this method the instance becomes
   * unusable: any in‑flight operations are aborted and subsequent invocations
   * of `run()` will throw.
   */
  public terminate(): void {
    if (this.terminated) {
      return;
    }
    this.terminated = true;

    this.hardAbort.abort();

    this.cancel();
  }

  public sessionId: string;

  constructor({
    model,
    instructions,
    approvalPolicy,
    config,
    onItem,
    onLoading,
    getCommandConfirmation,
    onLastResponseId,
  }: OllamaAgentLoopParams & { config?: AppConfig }) {
    this.model = model;
    this.instructions = instructions;
    this.approvalPolicy = approvalPolicy;

    // If no `config` has been provided we derive a minimal stub so that the
    // rest of the implementation can rely on `this.config` always being a
    // defined object.  We purposefully copy over the `model` and
    // `instructions` that have already been passed explicitly so that
    // downstream consumers (e.g. telemetry) still observe the correct values.
    this.config =
      config ??
      ({
        model,
        instructions: instructions ?? "",
      } as AppConfig);
    
    this.ollamaApiUrl = this.config.ollama?.apiUrl || "http://localhost:11434";
    
    this.onItem = onItem;
    this.onLoading = onLoading;
    this.getCommandConfirmation = getCommandConfirmation;
    this.onLastResponseId = onLastResponseId;
    this.sessionId = getSessionId() || randomUUID().replaceAll("-", "");

    setSessionId(this.sessionId);
    setCurrentModel(this.model);

    this.hardAbort = new AbortController();

    this.hardAbort.signal.addEventListener(
      "abort",
      () => this.execAbortController?.abort(),
      { once: true },
    );
  }

  private async handleFunctionCall(
    item: ResponseFunctionToolCall,
  ): Promise<Array<ResponseInputItem>> {
    // If the agent has been canceled in the meantime we should not perform any
    // additional work. Returning an empty array ensures that we neither execute
    // the requested tool call nor enqueue any follow‑up input items. This keeps
    // the cancellation semantics intuitive for users – once they interrupt a
    // task no further actions related to that task should be taken.
    if (this.canceled) {
      return [];
    }

    // ---------------------------------------------------------------------
    // Normalise the function‑call item into a consistent shape regardless of
    // whether it originated from the `/responses` or the `/chat/completions`
    // endpoint – their JSON differs slightly.
    // ---------------------------------------------------------------------

    const isChatStyle =
      // The chat endpoint nests function details under a `function` key.
      // We conservatively treat the presence of this field as a signal that
      // we are dealing with the chat format.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (item as any).function != null;

    const name: string | undefined = isChatStyle
      ? // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (item as any).function?.name
      : // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (item as any).name;

    const rawArguments: string | undefined = isChatStyle
      ? // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (item as any).function?.arguments
      : // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (item as any).arguments;

    // The OpenAI "function_call" item may have either `call_id` (responses
    // endpoint) or `id` (chat endpoint).  Prefer `call_id` if present but fall
    // back to `id` to remain compatible.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const callId: string = (item as any).call_id ?? (item as any).id;

    const args = parseToolCallArguments(rawArguments ?? "{}");
    if (isLoggingEnabled()) {
      log(
        `handleFunctionCall(): name=${
          name ?? "undefined"
        } callId=${callId} args=${rawArguments}`,
      );
    }

    if (args == null) {
      const outputItem: ResponseInputItem.FunctionCallOutput = {
        type: "function_call_output",
        call_id: item.call_id || callId, // Ensure we have a valid call_id
        output: `invalid arguments: ${rawArguments}`,
      };
      return [outputItem];
    }

    const outputItem: ResponseInputItem.FunctionCallOutput = {
      type: "function_call_output",
      // `call_id` is mandatory – ensure we never send `undefined` which would
      // trigger the "No tool output found…" 400 from the API.
      call_id: callId,
      output: "no function found",
    };

    // used to tell model to stop if needed
    const additionalItems: Array<ResponseInputItem> = [];

    // TODO: allow arbitrary function calls (beyond shell/container.exec)
    if (name === "container.exec" || name === "shell") {
      const {
        outputText,
        metadata,
        additionalItems: additionalItemsFromExec,
      } = await handleExecCommand(
        args,
        this.config,
        this.approvalPolicy,
        this.getCommandConfirmation,
        this.execAbortController?.signal,
      );
      outputItem.output = JSON.stringify({ output: outputText, metadata });

      if (additionalItemsFromExec) {
        additionalItems.push(...additionalItemsFromExec);
      }
    }

    return [outputItem, ...additionalItems];
  }

  /**
   * Extract thinking content from a response.
   * 
   * @param content The content to extract thinking from.
   * @returns An object containing the extracted thinking content and the cleaned content.
   */
  private extractThinkingContent(content: string): { 
    thinkingContent: string | null; 
    cleanedContent: string;
  } {
    // Check if the content contains thinking tags
    const thinkRegex = /<think>([\s\S]*?)<\/think>/;
    const match = content.match(thinkRegex);
    
    if (match) {
      // Extract the thinking content
      const thinkingContent = match[1].trim();
      
      // Remove the thinking tags from the content
      const cleanedContent = content.replace(thinkRegex, '').trim();
      
      return {
        thinkingContent,
        cleanedContent
      };
    }
    
    // No thinking content found
    return {
      thinkingContent: null,
      cleanedContent: content
    };
  }

  /**
   * Convert a response from Ollama to a format compatible with the OpenAI API.
   * 
   * @param response The response from Ollama.
   * @param thinkingStartTime The time when thinking started, used to calculate duration.
   * @returns An array of ResponseItems that can be used with the existing UI.
   */
  private convertOllamaResponseToResponseItem(
    response: OllamaChatCompletionResponse,
    thinkingStartTime: number,
  ): ResponseItem[] {
    // Generate a unique ID for this response
    const id = `ollama-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
    
    // Extract the content from the response
    const content = response.message.content;
    
    // Extract thinking content if present
    const { thinkingContent, cleanedContent } = this.extractThinkingContent(content);
    
    // Prepare the result array
    const result: ResponseItem[] = [];
    
    // Add reasoning item if thinking content was found
    if (thinkingContent) {
      const thinkingDuration = Date.now() - thinkingStartTime;
      
      // Create a reasoning item compatible with OpenAI's format
      // We need to cast to unknown first to avoid TypeScript errors
      const reasoningItem = {
        id: `reasoning-${id}`,
        type: "reasoning",
        duration_ms: thinkingDuration,
        content: thinkingContent,
      } as unknown as ResponseItem;
      
      result.push(reasoningItem);
    }
    
    // Check if the cleaned content contains a function call
    const functionCallMatch = cleanedContent.match(/```json\s*\{\s*"cmd"\s*:\s*\[\s*"([^"]+)"(?:,\s*"([^"]+)")?\s*\]\s*\}\s*```/);
    
    if (functionCallMatch && functionCallMatch[1]) {
      // This is a function call
      const functionName = functionCallMatch[1];
      const functionArgs = functionCallMatch[2] || "";
      
      result.push({
        id,
        type: "function_call",
        call_id: id,
        name: functionName,
        arguments: functionArgs,
      } as ResponseFunctionToolCall as unknown as ResponseItem);
    } else {
      // This is a regular message
      result.push({
        id,
        type: "message",
        role: "assistant",
        content: [
          {
            type: "output_text",
            text: cleanedContent,
            annotations: [],
          } as ResponseOutputText,
        ],
      } as unknown as ResponseItem);
    }
    
    return result;
  }

  /**
   * Parse function calls from the Ollama response text.
   * 
   * @param text The response text from Ollama.
   * @returns An array of function calls extracted from the text.
   */
  private parseFunctionCalls(text: string): Array<ResponseFunctionToolCall> {
    const functionCalls: Array<ResponseFunctionToolCall> = [];
    
    // Look for shell commands in the format: ```bash\ncommand\n```
    const shellCommandRegex = /```(?:bash|sh)\s*\n([\s\S]*?)\n```/g;
    let match;
    
    while ((match = shellCommandRegex.exec(text)) !== null) {
      const command = match[1].trim();
      if (command) {
        const id = `ollama-shell-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
        functionCalls.push({
          id,
          type: "function_call",
          call_id: id,
          name: "shell",
          arguments: JSON.stringify({ command: command.split(" ") }),
        } as ResponseFunctionToolCall);
      }
    }
    
    // Look for apply_patch commands in the format: ```json\n{"cmd":["apply_patch","*** Begin Patch\n..."]}\n```
    const applyPatchRegex = /```json\s*\{\s*"cmd"\s*:\s*\[\s*"apply_patch"\s*,\s*"([\s\S]*?)"\s*\]\s*\}\s*```/g;
    
    while ((match = applyPatchRegex.exec(text)) !== null) {
      const patchContent = match[1].replace(/\\n/g, "\n");
      if (patchContent) {
        const id = `ollama-patch-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
        functionCalls.push({
          id,
          type: "function_call",
          call_id: id,
          name: "apply_patch",
          arguments: JSON.stringify({ patch: patchContent }),
        } as ResponseFunctionToolCall);
      }
    }
    
    return functionCalls;
  }

  public async run(
    input: Array<ResponseInputItem>,
    previousResponseId: string = "",
  ): Promise<void> {
    try {
      if (this.terminated) {
        throw new Error("OllamaAgentLoop has been terminated");
      }
      
      // Record when we start "thinking" so we can report accurate elapsed time.
      const thinkingStart = Date.now();
      
      // Bump generation so that any late events from previous runs can be
      // identified and dropped.
      const thisGeneration = ++this.generation;

      // Reset cancellation flag and stream for a fresh run.
      this.canceled = false;
      this.currentStream = null;

      // Create a fresh AbortController for this run so that tool calls from a
      // previous run do not accidentally get signalled.
      this.execAbortController = new AbortController();
      if (isLoggingEnabled()) {
        log(
          `OllamaAgentLoop.run(): new execAbortController created (${this.execAbortController.signal}) for generation ${this.generation}`,
        );
      }

      let turnInput = [...input];

      // Display user messages in the UI before sending to Ollama
      for (const item of turnInput) {
        if (item.type === "message" && item.role === "user") {
          this.onItem(item as ResponseItem);
        }
      }

      this.onLoading(true);

      // Convert OpenAI-style input items to Ollama messages
      const newMessages: OllamaMessage[] = convertResponseItemsToOllamaMessages(turnInput as ResponseItem[]);
      
      // Combine with conversation history
      const messages: OllamaMessage[] = [...this.conversationHistory, ...newMessages];
      
      // Add system message with instructions if available
      if (this.instructions) {
        messages.unshift({
          role: "system",
          content: this.instructions,
        });
      }

      // Extract the actual model name without the "ollama:" prefix
      const actualModelName = extractOllamaModelName(this.model) || this.model;
      
      try {
        // Make the request to Ollama
        const response = await ollamaChatCompletion(actualModelName, messages, false);
        
        if (this.canceled || this.hardAbort.signal.aborted) {
          this.onLoading(false);
          return;
        }
        
        // Convert the response to a format compatible with the OpenAI API
        const responseItems = this.convertOllamaResponseToResponseItem(
          response as OllamaChatCompletionResponse,
          thinkingStart
        );
        
        // Emit each response item
        for (const item of responseItems) {
          this.onItem(item);
        }
        
        // Get the main response item (the last one, which is the message or function call)
        if (responseItems.length > 0) {
          const mainResponseItem = responseItems[responseItems.length - 1];
          
          // Check if the response contains function calls
          if (mainResponseItem && mainResponseItem.type === "message") {
            // Type guard to ensure we're working with a message item
            const messageItem = mainResponseItem as {
              type: string;
              content?: Array<{type: string; text?: string}>;
            };
            
            // Check if the message has content with text
            if (messageItem.content && 
                messageItem.content.length > 0 && 
                messageItem.content[0] && 
                'text' in messageItem.content[0] && 
                messageItem.content[0].text) {
              
              const content = messageItem.content[0].text;
              const functionCalls = this.parseFunctionCalls(content);
              
              // Process each function call
              for (const functionCall of functionCalls) {
                if (this.canceled || this.hardAbort.signal.aborted) {
                  this.onLoading(false);
                  return;
                }
                
                // Emit the function call
                this.onItem(functionCall as unknown as ResponseItem);
                
                // Handle the function call
                const result = await this.handleFunctionCall(functionCall);
                
                // Process the result
                for (const item of result) {
                  this.onItem(item as ResponseItem);
                }
              }
            }
          } else if (mainResponseItem && mainResponseItem.type === "function_call") {
            // Handle the function call
            const result = await this.handleFunctionCall(mainResponseItem as ResponseFunctionToolCall);
            
            // Process the result
            for (const item of result) {
              this.onItem(item as ResponseItem);
            }
          }
        }
        
        // Update conversation history with new messages and response
        this.updateConversationHistory(newMessages, response as OllamaChatCompletionResponse);
        
        // Generate a unique response ID
        const responseId = `ollama-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
        this.onLastResponseId(responseId);
        
      } catch (error) {
        // Handle errors
        console.error("Error in OllamaAgentLoop.run():", error);
        
        this.onItem({
          id: `error-${Date.now()}`,
          type: "message",
          role: "system",
          content: [
            {
              type: "input_text",
              text: `⚠️  Error while contacting Ollama: ${error instanceof Error ? error.message : String(error)}`,
            },
          ],
        });
      } finally {
        this.onLoading(false);
      }
    } catch (error) {
      console.error("Unhandled error in OllamaAgentLoop.run():", error);
      this.onLoading(false);
      
      this.onItem({
        id: `error-${Date.now()}`,
        type: "message",
        role: "system",
        content: [
          {
            type: "input_text",
            text: `⚠️  Unhandled error: ${error instanceof Error ? error.message : String(error)}`,
          },
        ],
      });
    }
  }
  
  /**
   * Update the conversation history with new messages and the model's response.
   * This maintains context between requests.
   * 
   * @param newMessages The new messages sent in this turn.
   * @param response The response from Ollama.
   */
  /**
   * Clear the conversation history.
   * This is used when the user issues the `/clear` command.
   * Note: For a complete reset, it's better to recreate the agent instance
   * rather than just clearing the conversation history.
   */
  public clearConversationHistory(): void {
    // Reset the conversation history to an empty array
    this.conversationHistory = [];
    
    // Reset the generation counter to ensure a fresh start
    this.generation = 0;
    
    // Reset the session ID to ensure a fresh start
    this.sessionId = randomUUID().replaceAll("-", "");
    setSessionId(this.sessionId);
    
    if (isLoggingEnabled()) {
      log(`OllamaAgentLoop.clearConversationHistory(): conversation history cleared, new session ID: ${this.sessionId}`);
    }
  }

  private updateConversationHistory(
    newMessages: OllamaMessage[],
    response: OllamaChatCompletionResponse
  ): void {
    // Add new messages to history
    this.conversationHistory.push(...newMessages);
    
    // Add model response to history
    this.conversationHistory.push(response.message);
    
    // Limit history size to prevent context overflow
    // Keep the most recent messages for context
    const MAX_HISTORY_LENGTH = 20;
    if (this.conversationHistory.length > MAX_HISTORY_LENGTH) {
      // Always keep the system message if it exists
      const systemMessages = this.conversationHistory.filter(msg => msg.role === "system");
      const nonSystemMessages = this.conversationHistory.filter(msg => msg.role !== "system");
      
      // Keep only the most recent non-system messages
      const recentMessages = nonSystemMessages.slice(-MAX_HISTORY_LENGTH + systemMessages.length);
      
      // Reconstruct history with system messages first, then recent messages
      this.conversationHistory = [...systemMessages, ...recentMessages];
    }
  }
}
