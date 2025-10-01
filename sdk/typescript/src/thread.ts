import { CodexOptions } from "./codexOptions";
import { ThreadEvent } from "./events";
import { CodexExec } from "./exec";
import { ThreadItem } from "./items";
import { TurnOptions } from "./turnOptions";

export type RunResult = {
  items: ThreadItem[];
  finalResponse: string;
};

export type RunStreamedResult = {
  events: AsyncGenerator<ThreadEvent>;
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
      workingDirectory: options?.workingDirectory,
      skipGitRepoCheck: options?.skipGitRepoCheck,
    });
    for await (const item of generator) {
      const parsed = JSON.parse(item) as ThreadEvent;
      if (parsed.type === "thread.started") {
        this.id = parsed.thread_id;
      }
      yield parsed;
    }
  }

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
