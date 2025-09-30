import { CodexOptions } from "./codexOptions";
import { ConversationEvent } from "./events";
import { CodexExec } from "./exec";
import { ConversationItem } from "./items";

export type RunResult = {
  items: ConversationItem[];
  finalResponse: string;
};

export type RunStreamedResult = {
  events: AsyncGenerator<ConversationEvent>;
};

export type Input = string;

export class Thread {
  private exec: CodexExec;
  private options: CodexOptions;
  public id: string | null;

  constructor(exec: CodexExec, options: CodexOptions, id: string | null = null) {
    this.exec = exec;
    this.options = options;
    this.id = id;
  }

  async runStreamed(input: string): Promise<RunStreamedResult> {
    return { events: this.runStreamedInternal(input) };
  }

  private async *runStreamedInternal(input: string): AsyncGenerator<ConversationEvent> {
    const generator = this.exec.run({
      input,
      baseUrl: this.options.baseUrl,
      apiKey: this.options.apiKey,
      sessionId: this.id,
    });
    for await (const item of generator) {
      const parsed = JSON.parse(item) as ConversationEvent;
      if (parsed.type === "session.created") {
        this.id = parsed.session_id;
      }
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
