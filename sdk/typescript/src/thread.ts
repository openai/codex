import { CodexOptions } from "./codexOptions";
import { ThreadEvent } from "./events";
import { CodexExec } from "./exec";
import { ThreadItem } from "./items";

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
  async runStreamed(input: string): Promise<StreamedTurn> {
    return { events: this.runStreamedInternal(input) };
  }

  /** Provides the input to the agent and returns the completed turn.
   * Throws if the turn fails.
   */
  async run(input: string): Promise<Turn> {
    const generator = this.runStreamedInternal(input);
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


  private async *runStreamedInternal(input: string): AsyncGenerator<ThreadEvent> {
    const generator = this._exec.run({
      input,
      baseUrl: this._options.baseUrl,
      apiKey: this._options.apiKey,
      threadId: this._id,
    });
    for await (const item of generator) {
      const parsed = JSON.parse(item) as ThreadEvent;
      if (parsed.type === "thread.started") {
        this._id = parsed.thread_id;
      }
      yield parsed;
    }
  }

}
