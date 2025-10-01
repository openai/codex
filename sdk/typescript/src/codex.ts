import { CodexOptions } from "./codexOptions";
import { CodexExec } from "./exec";
import { Thread } from "./thread";

/**
 * Codex is the main class for interacting with the Codex agent.
 *
 * Use the `startThread()` method to start a new thread or `resumeThread()` to resume a previously started thread.
 */
export class Codex {
  private exec: CodexExec;
  private options: CodexOptions;

  constructor(options: CodexOptions = {}) {
    this.exec = new CodexExec(options.codexPathOverride);
    this.options = options;
  }

  /**
   * Starts a new conversation with an agent.
   * @returns A new thread instance.
   */
  startThread(): Thread {
    return new Thread(this.exec, this.options);
  }

  /**
   * Resumes a conversation with an agent based on the thread id.
   * Threads are persisted in ~/.codex/sessions.
   *
   * @param id The id of the thread to resume.
   * @returns A new thread instance.
   */
  resumeThread(id: string): Thread {
    return new Thread(this.exec, this.options, id);
  }
}
