import { CodexOptions } from "./codexOptions";
import { ThreadEvent } from "./events";
import { CodexExec } from "./exec";
import { ThreadItem } from "./items";
import { TurnOptions } from "./turnOptions";

/** Completed turn. */
export type Turn = {
  items: ThreadItem[];
  finalResponse: string;
};

/** The result of the `runStreamed` method. */
export type StreamedTurn = {
  events: AsyncGenerator<ThreadEvent>;
};

/** An input to send to the agent. */
export type Input = string;

/** Respesent a thread of conversation with the agent. One thread can have multiple consecutive turns. */
export class Thread {
  private _exec: CodexExec;
  private _options: CodexOptions;
  private _id: string | null;

  /** Returns the ID of the thread. Populated after the first turn starts. */
  public get id(): string | null {
    return this.id;
  }

  constructor(exec: CodexExec, options: CodexOptions, id: string | null = null) {
    this._exec = exec;
    this._options = options;
    this._id = id;
  }

  /** Provides the input to the agent and streams events as they are produced during the turn. */
  async runStreamed(input: string, options?: TurnOptions): Promise<RunStreamedResult> {
    return { events: this.runStreamedInternal(input, options) };
  }

  private async *runStreamedInternal(
    input: string,
    options?: TurnOptions,
  ): AsyncGenerator<ThreadEvent> {
    const generator = this.exec.run({
      input,
      baseUrl: this.options.baseUrl,
      apiKey: this.options.apiKey,
      threadId: this.id,
      model: options?.model,
      sandboxMode: options?.sandboxMode,
    });
    for await (const item of generator) {
      const parsed = JSON.parse(item) as ThreadEvent;
      if (parsed.type === "thread.started") {
        this.id = parsed.thread_id;
      }
      yield parsed;
    }
  }

  /** Provides the input to the agent and returns the completed turn.
  async run(input: string, options?: TurnOptions): Promise<RunResult> {
    const generator = this.runStreamedInternal(input, options);
    const items: ThreadItem[] = [];
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
