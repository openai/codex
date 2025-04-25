import os from "os";
import pty from "node-pty";
import readline from "readline";
import OpenAI from "openai";
import type { AppConfig } from "../utils/config";
import { getSecuritySystemPrompt, getOffensiveSystemPrompt, getPredatorSystemPrompt } from "./index";
import { detectTools, isToolInstalled } from "./tools/detection";

// Simple console spinner for "thinking" indicator
const loaderFrames = ['⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏'];
function startSpinner(label = 'Thinking') {
  let idx = 0;
  process.stdout.write(label + ' ' + loaderFrames[0]);
  const id = setInterval(() => {
    idx = (idx + 1) % loaderFrames.length;
    process.stdout.write('\r' + label + ' ' + loaderFrames[idx]);
  }, 80);
  return () => {
    clearInterval(id);
    process.stdout.write('\r');
  };
}

/**
 * Fully interactive Adversys session using a pseudo-terminal and streaming chat.
 */
import net from "net";
import { once } from "events";
import { installTool } from "./utils/install-tool";
import fs from "fs";
// LLM function definitions for the autonomous loop
const summarizeOutputFn = {
  name: "summarizeOutput",
  description: "Summarize raw tool output in 2–3 bullet points.",
  parameters: {
    type: "object",
    properties: { output: { type: "string" } },
    required: ["output"],
  },
};
const proposeNextCmdFn = {
  name: "proposeNextCmd",
  description: "Propose the next security test as a command proposal.",
  parameters: {
    type: "object",
    properties: { summary: { type: "array", items: { type: "string" } } },
    required: ["summary"],
  },
};
/**
 * Connect to the shell-agent IPC socket, retrying until available.
 */
/**
 * Connect to the shell-agent IPC socket, retrying until available.
 */
export async function connectIpc(path: string): Promise<net.Socket> {
  while (true) {
    try {
      // create socket and wait for either 'connect' or 'error'
      let client: net.Socket;
      await new Promise<void>((resolve, reject) => {
        client = net.createConnection({ path });
        client.once('connect', () => resolve());
        client.once('error', (err) => reject(err));
      });
      console.log(`Connected to shell-agent at ${path}`);
      return client!;
    } catch (err: any) {
      // Retry if the socket is not yet ready or refused
      if (err.code === 'ECONNREFUSED' || err.code === 'ENOENT') {
        console.log(`Waiting for shell-agent at ${path}…`);
        // retry after short delay
        await new Promise(r => setTimeout(r, 500));
        continue;
      }
      // Other errors bubble up
      throw err;
    }
  }
}
export async function runInteractiveSession(
  prompt: string | undefined,
  config: AppConfig,
  options: { target?: string; session?: string; offensive?: boolean; predator?: boolean; ipcSocket?: string; noInstall?: boolean; imagePaths?: string[] }
): Promise<void> {
  // Session state for custom REPL commands and summarization
  const historyRecords: Array<{ cmd: string; output: string; exitCode?: number }> = [];
  // Session log: each entry records cmd, exitCode, and output
  const session: Array<{ cmd: string; exitCode?: number; output?: string }> = [];
  /**
   * Execute -> Observe -> Decide loop for one command
   */
  async function executeObserveDecide(cmd: string) {
    // 2.1: record initial command
    session.push({ cmd });
    // 2.2: send run request
    console.log("[SmokeTest] Running:", cmd);
    ipcClient?.write(JSON.stringify({ type: 'run', cmd }) + '\n');
    // 2.3: await exit and collect output
    const result = await new Promise<{ exitCode: number; data: string }>(resolve => {
      let buffer = '';
      const onMessage = (raw: string) => {
        try {
          const msg = JSON.parse(raw);
          if (msg.type === 'output') buffer += msg.data;
          if (msg.type === 'exit') {
            ipcClient?.removeListener('data', onMessage);
            resolve({ exitCode: msg.exitCode, data: buffer });
          }
        } catch {}
      };
      ipcClient?.on('data', onMessage);
    });
    // 2.4: merge result into session
    const last = session[session.length - 1];
    last.exitCode = result.exitCode;
    last.output = result.data;
    // 2.5: Summarize output using LLM function
    try {
      const sumRes = await openai.chat.completions.create({
        model: config.model,
        messages: messages as any,
        functions: [summarizeOutputFn],
        function_call: { name: "summarizeOutput" },
      });
      const args = sumRes.choices?.[0]?.message?.function_call?.arguments;
      if (args) {
        const { output: sumText } = JSON.parse(args);
        console.log("\n=== Summary ===");
        sumText.split(/\r?\n/).forEach(line => {
          if (line.trim()) console.log("•", line.trim());
        });
        messages.push({ role: 'assistant', content: sumText });
      }
    } catch (e: any) {
      console.error('[ERROR][Chat] summarizeOutput failed:', e.message || e);
    }
    // 2.6: Propose next command via LLM function
    try {
      const propRes = await openai.chat.completions.create({
        model: config.model,
        messages: messages as any,
        functions: [proposeNextCmdFn],
        function_call: { name: "proposeNextCmd" },
      });
      const propArgs = propRes.choices?.[0]?.message?.function_call?.arguments;
      if (propArgs) {
        const { cmd: nextCmd, type } = JSON.parse(propArgs);
        if (type === 'cmd-proposal') {
          console.log(`LLM proposes next: ${nextCmd}`);
          const ok = await new Promise<boolean>(res =>
            rl.question('Approve next command? [Y/n] ', a => res(/^y(es)?$/i.test(a.trim())))
          );
          if (ok) {
            return executeObserveDecide(nextCmd);
          }
        }
      }
    } catch (e: any) {
      console.error('[ERROR][Chat] proposeNextCmd failed:', e.message || e);
    }
    return last;
  }
  let lastOutputBuffer = '';
  let awaitingSummarization = false;
  // IPC client placeholder
  let ipcClient: net.Socket | undefined;
  if (options.ipcSocket) {
    ipcClient = await connectIpc(options.ipcSocket);
    ipcClient.setEncoding('utf8');
    ipcClient.on('data', (chunk: string) => {
      const lines = chunk.split('\n');
      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed) continue;
        let msg: any;
        try { msg = JSON.parse(trimmed); }
        catch (e) { console.error('[ERROR][Chat] IPC parse error:', e); continue; }
        if (msg.type === 'output' && typeof msg.data === 'string') {
          // process.stdout.write(msg.data)  // raw output forwarded to terminal
          if (awaitingSummarization) lastOutputBuffer += msg.data;
        } else if (msg.type === 'exit') {
          const exitCode = msg.exitCode;
          // Update history record
          if (historyRecords.length > 0) {
            const last = historyRecords[historyRecords.length - 1];
            last.output = lastOutputBuffer;
            last.exitCode = exitCode;
          }
          // Command finished, trigger summarization
          if (awaitingSummarization) {
            awaitingSummarization = false;
            summarizeAndPropose();
          }
        }
      }
    });
    ipcClient.on('error', err => console.error('IPC connection error:', err));
  }
  // Spawn PTY if not using IPC
  let ptyProcess: ReturnType<typeof pty.spawn> | undefined;
  if (!ipcClient) {
    const shell = process.env.SHELL || (os.platform() === 'win32' ? 'powershell.exe' : 'bash');
    ptyProcess = pty.spawn(shell, ['-i'], { name: 'xterm-color', cwd: process.cwd(), env: process.env });
    ptyProcess.on('data', (data) => process.stdout.write(data));
  }
  // Bootstrap tool inventory
  await detectTools();

  // Prepare chat history with appropriate system prompt
  const systemPrompt = options.predator
    ? getPredatorSystemPrompt()
    : options.offensive
      ? getOffensiveSystemPrompt()
      : getSecuritySystemPrompt();
  // Initialize chat history
  const messages: Array<{ role: string; content: string }> = [
    { role: 'system', content: systemPrompt }
  ];
  // Include any images the user provided
  if (options.imagePaths && options.imagePaths.length > 0) {
    for (const imgPath of options.imagePaths) {
      if (fs.existsSync(imgPath)) {
        messages.push({ role: 'user', content: `Image provided: ${imgPath}` });
      } else {
        console.warn(`[WARN][Chat] Image path not found: ${imgPath}`);
      }
    }
  }
  if (options.target) {
    messages.push({ role: 'system', content: `Target: ${options.target}` });
  }
  if (prompt) {
    messages.push({ role: 'user', content: prompt });
  }

  // Initialize OpenAI client
  const openai = new OpenAI({ apiKey: config.apiKey });
  // Automated first reconnaissance if target provided
  if (options.target) {
    const autoMsg = `Please perform an initial reconnaissance scan (e.g., nmap) against the target.`;
    messages.push({ role: 'user', content: autoMsg });
    let assistantReply = '';
    try {
      // show spinner while LLM prepares reconnaissance instructions
      const stopReconSpinner = startSpinner('Reconnaissance');
      const stream = await openai.chat.completions.create({
        model: config.model,
        messages: messages as any,
        stream: true
      });
      stopReconSpinner();
      for await (const part of stream) {
        const delta = part.choices?.[0]?.delta?.content;
        if (delta) {
          process.stdout.write(delta);
          assistantReply += delta;
        }
      }
      console.log();
      messages.push({ role: 'assistant', content: assistantReply });
      // Detect a backtick‐wrapped command
      const cmdMatch = assistantReply.match(/`([^`]+)`/);
      if (cmdMatch) {
        const cmd = cmdMatch[1].trim();
        const toolName = cmd.split(/\s+/)[0];
        if (!isToolInstalled(toolName)) {
          if (options.noInstall) {
            console.log(`[WARN][Chat] Tool '${toolName}' not installed; skipping auto-install (--no-install)`);
          } else {
            console.log(`[DEBUG][Chat] Tool '${toolName}' not installed; installing now...`);
            try {
              await installTool(toolName);
            } catch (err: any) {
              console.error(`[ERROR][Chat] Failed to install '${toolName}':`, err.message || err);
            }
          }
        }
        const ok = await new Promise<boolean>(res =>
          rl.question(`Execute command: ${cmd}? (y/N) `, ans => res(/^y(es)?$/i.test(ans.trim())))
        );
        if (ok) {
          console.log("[DEBUG][Chat] Sending run request:", cmd);
          if (ipcClient) ipcClient.write(JSON.stringify({ type: 'run', cmd }) + '\n');
          else ptyProcess!.write(cmd + '\r\n');
        }
      }
    } catch (e: any) {
      console.error('Chat error:', e.message ?? e);
    }
  }
  // Set up readline with mode‑aware prompt
  const modeLabel = options.predator
    ? 'predator'
    : options.offensive
      ? 'offensive'
      : 'pentest';
  const promptLabel = `adversys(${modeLabel})> `;
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  rl.setPrompt(promptLabel);
  rl.prompt();
  // Summarization & next-step proposal after command exit
  async function summarizeAndPropose() {
    // Inject raw shell output into chat context
    messages.push({ role: 'system', content: `Shell output:\n${lastOutputBuffer}` });
    // Ask LLM for summary and next command
    const summaryRequest =
      `Summarize the above output in 2–3 bullet points, then propose the next command as JSON: { "type":"cmd-proposal", "cmd":"..." }`;
    messages.push({ role: 'user', content: summaryRequest });
    // Ask LLM to summarize and propose next command
    console.log('Summarizing and proposing next command...');
    // Spinner during summarization
    const stopSumSpinner = startSpinner('Summarizing');
    try {
      stopSumSpinner();
      const resp = await openai.chat.completions.create({
        model: config.model,
        messages: messages as any,
      });
      const content = resp.choices?.[0]?.message?.content;
      if (!content) return;
      // Separate summary and JSON proposal
      const firstBrace = content.indexOf('{');
      let summaryText = content;
      let proposalJson = '';
      if (firstBrace >= 0) {
        summaryText = content.slice(0, firstBrace).trim();
        proposalJson = content.slice(firstBrace).trim();
      }
      console.log('\n=== Summary ===\n' + summaryText + '\n');
      if (proposalJson) {
        let proposal;
        try { proposal = JSON.parse(proposalJson); }
        catch (e) { console.error('Invalid JSON proposal:', e); return; }
        console.log(`LLM proposes next: ${proposal.cmd}`);
        const ok = await new Promise<boolean>(res =>
          rl.question('Approve next command? [Y/n] ', ans => res(/^y(es)?$/i.test(ans.trim())))
        );
        if (ok) {
          if (ipcClient) ipcClient.write(JSON.stringify({ type: 'run', cmd: proposal.cmd }) + '\n');
          else ptyProcess!.write(proposal.cmd + '\r');
        }
      }
      // Record assistant reply
      messages.push({ role: 'assistant', content });
    } catch (e: any) {
      console.error('[ERROR][Chat] Summarization error:', e.message || e);
    }
  }

  for await (const line of rl) {
    const userText = line.trim();
    // Session commands
    if (userText === '/history') {
      if (historyRecords.length === 0) console.log('No commands in session history.');
      else historyRecords.forEach((h, i) => {
        const outPreview = (h.output || '').split(/\r?\n/)[0] || '';
        console.log(`${i+1}. ${h.cmd} → exit ${h.exitCode ?? '?'}; output: ${outPreview}`);
      });
      rl.prompt();
      continue;
    }
    if (userText === '/last-output') {
      console.log(lastOutputBuffer || '<no output>');
      rl.prompt();
      continue;
    }
    if (userText.startsWith('/run ')) {
      const cmd = userText.slice(5).trim();
      if (!cmd) { rl.prompt(); continue; }
      // Record and execute
      historyRecords.push({ cmd, output: '', exitCode: undefined });
      lastOutputBuffer = '';
      awaitingSummarization = true;
      if (ipcClient) ipcClient.write(JSON.stringify({ type: 'run', cmd }) + '\n');
      else ptyProcess!.write(cmd + '\r');
      rl.prompt();
      continue;
    }
    if (!userText) { rl.prompt(); continue; }
    if (/^(exit|quit)$/i.test(userText)) return process.exit(0);
    if (/^help$/i.test(userText)) { console.log('Commands: help, exit, or ask security tasks'); rl.prompt(); continue; }
    // Persona response for identity queries
    if (/who are you\??/i.test(userText) || /your name\??/i.test(userText)) {
      let persona: string;
      if (options.predator) {
        persona = 'Adversys Predator Mode, the fully autonomous adversary agent.';
      } else if (options.offensive) {
        persona = 'Adversys Offensive Cyber Agent, your Kill Chain specialist.';
      } else {
        persona = 'Adversys Pentest Security Agent, your structured vulnerability assistant.';
      }
      const resp = `Hi, I am ${persona}`;
      console.log(resp);
      messages.push({ role: 'assistant', content: resp });
      rl.prompt();
      continue;
    }

    // Add user message to history
    messages.push({ role: 'user', content: userText });

    // Stream assistant reply
    let assistantReply = '';
    try {
      // show spinner while waiting for response
      const stopThinkSpinner = startSpinner('Thinking');
      const stream = await openai.chat.completions.create({
        model: config.model,
        messages: messages as any,
        stream: true
      });
      stopThinkSpinner();
      for await (const part of stream) {
        const delta = part.choices?.[0]?.delta?.content;
        if (delta) { process.stdout.write(delta); assistantReply += delta; }
      }
      console.log();
    messages.push({ role: 'assistant', content: assistantReply });
    // Detect JSON command proposal from LLM
    let proposal: { type: string; cmd: string } | null = null;
    try {
      const parsed = JSON.parse(assistantReply.trim());
      if (parsed && parsed.type === 'cmd-proposal' && typeof parsed.cmd === 'string') {
        proposal = parsed;
      }
    } catch {
      // not a JSON proposal
    }
    if (proposal) {
      console.log(`LLM proposes: ${proposal.cmd}`);
      const ok = await new Promise<boolean>(res =>
        rl.question('Approve? [Y/n] ', ans => res(/^y(es)?$/i.test(ans.trim())))
      );
      if (ok) {
        const entry = await executeObserveDecide(proposal.cmd);
        console.log('[SmokeTest] Ran:', entry.cmd, 'exit=', entry.exitCode);
        const snippet = entry.output
          ?.trim()
          .replace(/\s+/g, ' ')
          .slice(0, 100) || '';
        console.log('[SmokeTest] Output snippet:', snippet);
      }
      rl.prompt();
      continue;
    }
    } catch (e: any) {
      console.error('Chat error:', e.message ?? e);
    }

    // Detect a backtick‑wrapped command in the reply
    const cmdMatch = assistantReply.match(/`([^`]+)`/);
    if (cmdMatch) {
      let cmd = cmdMatch[1].trim();
      // Auto‑install missing tool
      const toolName = cmd.split(/\s+/)[0];
      if (!isToolInstalled(toolName)) {
        console.log(`Tool '${toolName}' not found; auto‑installing...`);
        let installCmd;
        if (os.platform() === 'darwin') installCmd = `brew install ${toolName}`;
        else if (os.platform() === 'linux') installCmd = `sudo apt-get update && sudo apt-get install -y ${toolName}`;
        else throw new Error(`Auto‑install unsupported on this OS`);
        ptyProcess.write(installCmd + '\r');
        await new Promise(r => setTimeout(r, 10000));
        await detectTools();
      }
      // Confirm and run
      const ok = await new Promise<boolean>(res =>
        rl.question(`Execute command: ${cmd}? (y/N) `, ans => res(/^y(es)?$/i.test(ans.trim())))
      );
      if (ok) {
        // Prepare to capture output for summarization
        lastOutputBuffer = '';
        awaitingSummarization = true;
        // Send run command via IPC or PTY
        console.log("[DEBUG][Chat] Sending run request:", cmd);
        if (ipcClient) ipcClient.write(JSON.stringify({ type: 'run', cmd }) + '\n');
        else ptyProcess!.write(cmd + '\r');
      }
    }

    rl.prompt();
  }
}