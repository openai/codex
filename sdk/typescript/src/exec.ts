import { spawn } from "child_process";
import readline from 'node:readline';

export type CodexExecArgs = {
    input: string;
}

export class CodexExec {
    private executablePath: string; 
    private  baseUrl: string;
    constructor(executablePath: string, baseUrl: string) {
        this.executablePath = executablePath;
        this.baseUrl = baseUrl;
    }

    async *run(args: CodexExecArgs): AsyncGenerator<string> {
        const child = spawn(this.executablePath, ["exec", "--experimental-json", args.input], {
            env: {
                ...process.env,
                OPENAI_BASE_URL: this.baseUrl,
                OPENAI_API_KEY: "test",
            },
        });
        

        let spawnError: unknown | null = null;
        child.once("error", (err) => (spawnError = err));
      
        if (!child.stdout) {
          child.kill();
          throw new Error("Child process has no stdout");
        }

        const rl = readline.createInterface({
          input: child.stdout,
          crlfDelay: Infinity,
        });
      
        try {
          for await (const line of rl) {
            // `line` is a string (Node sets default encoding to utf8 for readline)
            yield line as string;
          }
      
          // Wait for actual exit after streams close
          const exitCode: number | null = await new Promise((resolve) => {
            child.once("exit", (code) => resolve(code));
          });
      
          if (spawnError) throw spawnError;
          if ((exitCode ?? 0) !== 0) {
            throw new Error(`Codex Exec exited with code ${exitCode}`);
          }
        } finally {
          rl.close();
          child.removeAllListeners();
          try {
            if (!child.killed) child.kill();
          } catch {
            // ignore
          }
        }
    }
}
