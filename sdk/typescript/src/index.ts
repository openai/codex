import { ConversationEvent } from "./events.js";
import { CodexExec } from "./exec.js";
import { ConversationItem } from "./items.js";

export type {
  ConversationEvent,
  SessionCreatedEvent,
  TurnStartedEvent,
  TurnCompletedEvent,
  ItemStartedEvent,
  ItemUpdatedEvent,
  ItemCompletedEvent,
  ConversationErrorEvent,
} from "./events.js";
export type {
  ConversationItem,
  AssistantMessageItem,
  ReasoningItem,
  CommandExecutionItem,
  FileChangeItem,
  McpToolCallItem,
  WebSearchItem,
  TodoListItem,
  ErrorItem,
} from "./items.js";

export type CodexOptions = {
  // TODO: remove
  executablePath: string;
  // TODO: remove
  baseUrl: string;
  apiKey: string;
};

export class Codex {
  private exec: CodexExec;
  private options: CodexOptions;

  constructor(options: CodexOptions) {
    if (!options.executablePath) {
      throw new Error("executablePath is required");
    }

    this.exec = new CodexExec(options.executablePath);
    this.options = options;
  }

  createConversation(): Conversation {
    return new Conversation(this.exec, this.options);
  }
}

export type RunResult = {
  items: ConversationItem[];
  finalResponse: string;
};

export type RunStreamedResult = {
  events: AsyncGenerator<ConversationEvent>;
};

export type Input = string;

export class Conversation {
  private exec: CodexExec;
  private options: CodexOptions;
  constructor(exec: CodexExec, options: CodexOptions) {
    this.exec = exec;
    this.options = options;
  }

  async runStreamed(input: string): Promise<RunStreamedResult> {
    return { events: this.runStreamedInternal(input) };
  }

  private async *runStreamedInternal(input: string): AsyncGenerator<ConversationEvent> {
    const generator = this.exec.run({
      input,
      baseUrl: this.options.baseUrl,
      apiKey: this.options.apiKey,
    });
    for await (const item of generator) {
      const parsed = JSON.parse(item) as ConversationEvent;
      yield parsed;
    }
  }

  async run(input: string): Promise<RunResult> {
    const generator = this.runStreamedInternal(input);
    const items: ConversationItem[] = [];
    let finalResponse: string = "";
    for await (const event of generator) {
      if (event.type === "item.completed") {
        if (event.item.item_type === "assistant_message") {
          finalResponse = event.item.text;
        }
        items.push(event.item);
      }
    }
    return { items, finalResponse };
  }
}
