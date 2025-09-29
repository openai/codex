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
      const parsed = JSON.parse(item);
      finalResponse = getFinalResponse(parsed) ?? finalResponse;
      items.push(parsed);
    }
    return {
      items,
      finalResponse,
    };
  }
}

const getFinalResponse = (event: ConversationEvent): string | null => {
  if (event.type === "item.completed" && event.item.item_type === "assistant_message") {
    return event.item.text;
  }
  return null;
}
