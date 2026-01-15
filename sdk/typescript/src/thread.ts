import { CodexOptions } from "./codexOptions";
import { ThreadEvent, ThreadError, Usage } from "./events";
import { CodexExec } from "./exec";
import { ThreadItem } from "./items";
import { ThreadOptions } from "./threadOptions";
import { TurnOptions } from "./turnOptions";
import { createOutputSchemaFile } from "./outputSchemaFile";

/** Completed turn. */
export type Turn = {
  items: ThreadItem[];
  finalResponse: string;
  usage: Usage | null;
};

/** Alias for `Turn` to describe the result of `run()`. */
export type RunResult = Turn;

/** The result of the `runStreamed` method. */
export type StreamedTurn = {
  events: AsyncGenerator<ThreadEvent>;
};

/** Alias for `StreamedTurn` to describe the result of `runStreamed()`. */
export type RunStreamedResult = StreamedTurn;

/** An input to send to the agent. */
export type UserInput =
  | {
      type: "text";
      text: string;
    }
  | {
      type: "local_image";
      path: string;
    };

export type Input = string | UserInput[];

/** Respesent a thread of conversation with the agent. One thread can have multiple consecutive turns. */
export class Thread {
  private _exec: CodexExec;
  private _options: CodexOptions;
  private _id: string | null;
  private _threadOptions: ThreadOptions;

  /** Returns the ID of the thread. Populated after the first turn starts. */
  public get id(): string | null {
    return this._id;
  }

  /* @internal */
  constructor(
    exec: CodexExec,
    options: CodexOptions,
    threadOptions: ThreadOptions,
    id: string | null = null,
  ) {
    this._exec = exec;
    this._options = options;
    this._id = id;
    this._threadOptions = threadOptions;
  }

  /** Provides the input to the agent and streams events as they are produced during the turn. */
  async runStreamed(input: Input, turnOptions: TurnOptions = {}): Promise<StreamedTurn> {
    return { events: this.runStreamedInternal(input, turnOptions) };
  }

  private async *runStreamedInternal(
    input: Input,
    turnOptions: TurnOptions = {},
  ): AsyncGenerator<ThreadEvent> {
    const { schemaPath, cleanup } = await createOutputSchemaFile(turnOptions.outputSchema);
    const options = this._threadOptions;
    const { prompt, images } = normalizeInput(input);
    const generator = this._exec.run({
      input: prompt,
      baseUrl: this._options.baseUrl,
      apiKey: this._options.apiKey,
      threadId: this._id,
      images,
      model: options?.model,
      sandboxMode: options?.sandboxMode,
      workingDirectory: options?.workingDirectory,
      skipGitRepoCheck: options?.skipGitRepoCheck,
      outputSchemaFile: schemaPath,
      modelReasoningEffort: options?.modelReasoningEffort,
      signal: turnOptions.signal,
      networkAccessEnabled: options?.networkAccessEnabled,
      webSearchMode: options?.webSearchMode,
      webSearchEnabled: options?.webSearchEnabled,
      approvalPolicy: options?.approvalPolicy,
      additionalDirectories: options?.additionalDirectories,
    });
    try {
      for await (const item of generator) {
        let parsed: ThreadEvent;
        try {
          parsed = JSON.parse(item) as ThreadEvent;
        } catch (error) {
          throw new Error(`Failed to parse item: ${item}`, { cause: error });
        }
        if (parsed.type === "thread.started") {
          this._id = parsed.thread_id;
        }
        yield parsed;
      }
    } finally {
      await cleanup();
    }
  }

  /** Provides the input to the agent and returns the completed turn. */
  async run(input: Input, turnOptions: TurnOptions = {}): Promise<Turn> {
    const generator = this.runStreamedInternal(input, turnOptions);
    const items: ThreadItem[] = [];
    let finalResponse: string = "";
    let usage: Usage | null = null;
    let turnFailure: ThreadError | null = null;
    for await (const event of generator) {
      if (event.type === "item.completed") {
        if (event.item.type === "agent_message") {
          finalResponse = event.item.text;
        }
        items.push(event.item);
      } else if (event.type === "turn.completed") {
        usage = event.usage;
      } else if (event.type === "turn.failed") {
        turnFailure = event.error;
        break;
      }
    }
    if (turnFailure) {
      throw new Error(turnFailure.message);
    }
    return { items, finalResponse, usage };
  }
}

function normalizeInput(input: Input): { prompt: string; images: string[] } {
  if (typeof input === "string") {
    return { prompt: input, images: [] };
  }
  const promptParts: string[] = [];
  const images: string[] = [];
  for (const item of input) {
    if (item.type === "text") {
      promptParts.push(item.text);
    } else if (item.type === "local_image") {
      images.push(item.path);
    }
  }
  return { prompt: promptParts.join("\n\n"), images };
}
