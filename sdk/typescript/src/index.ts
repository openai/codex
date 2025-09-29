import { ConversationEvent } from './events.js';
import { CodexExec } from './exec.js';
import { ConversationItem } from './items.js';

export type {
  ConversationEvent,
  SessionCreatedEvent,
  TurnStartedEvent,
  TurnCompletedEvent,
  ItemStartedEvent,
  ItemUpdatedEvent,
  ItemCompletedEvent,
  ConversationErrorEvent,
} from './events.js';
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
} from './items.js';

export type CodexOptions = {
  // TODO: remove
  executablePath: string;
  // TODO: remove
  baseUrl: string;
};

export class Codex {
  private exec: CodexExec;
  constructor(options: CodexOptions) {
    if (!options.executablePath) {
      throw new Error('executablePath is required');
    }

    this.exec = new CodexExec(options.executablePath, options.baseUrl);
  }

  createConversation(): Conversation {
    return new Conversation(this.exec);
  }
}

export type ConversationRunResult = {
  items: ConversationItem[];
  finalResponse: string;
};

export class Conversation {
  private exec: CodexExec;

  constructor(exec: CodexExec) {
    this.exec = exec;
  }

  async run(input: string): Promise<ConversationRunResult> {
    const generator = this.exec.run({ input });
    const items: ConversationItem[] = [];
    let finalResponse: string = '';
    for await (const item of generator) {
      const parsed = JSON.parse(item) as ConversationEvent;
      if (parsed.type === "item.completed"){
        if (parsed.item.item_type === "assistant_message") {
          finalResponse = parsed.item.text;
        }
        items.push(parsed.item);
      }
    }
    return {
      items,
      finalResponse,
    };
  }
}

