import { CodexOptions } from "./codexOptions";
import { CodexExec } from "./exec";
import { Thread } from "./thread";

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

  startThread(): Thread {
    return new Thread(this.exec, this.options);
  }

  resumeThread(id: string): Thread {
    return new Thread(this.exec, this.options, id);
  }
}
