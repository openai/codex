import os from "os";
import pty from "node-pty";
import fs from "fs";
import yaml from "js-yaml";
import { parseStringPromise } from "xml2js";
import path, { dirname } from "path";
import { fileURLToPath } from "url";
import OpenAI from "openai";
import type { AppConfig } from "../utils/config";
import { detectTools, isToolInstalled } from "./tools/detection";
import { installTool } from "./utils/install-tool";
import { recordCommand } from "./storage/sessions";
// Normalize a URL target by stripping scheme and trailing slash
function normalizeTarget(url: string): string {
  return url.replace(/^https?:\/\//, "").replace(/\/$/, "");
}

// Spinner for thinking indicator
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

// OWASP Top 10 (2021)
const OWASP_TOP10 = [
  'Injection',
  'Broken Authentication',
  'Sensitive Data Exposure',
  'XML External Entities (XXE)',
  'Broken Access Control',
  'Security Misconfiguration',
  'Cross-Site Scripting (XSS)',
  'Insecure Deserialization',
  'Using Components with Known Vulnerabilities',
  'Insufficient Logging & Monitoring'
];

/**
 * Fully automated Predator Mode: runs detection and exploitation for each OWASP Top 10
 */
export async function runPredatorSession(
  prompt: string | undefined,
  config: AppConfig,
  options: { target?: string; session?: string; offensive?: boolean; predator?: boolean; playbook?: string; dryRun?: boolean; generatePlaybook?: boolean; mission?: string; ipcSocket?: string; noInstall?: boolean; imagePaths?: string[] }
): Promise<void> {
  const target = options.target || '<unspecified>';
  // Normalize target for command placeholders
  const normalizedTarget = normalizeTarget(target);
  // Derive hostname for tools that require host, not full URL
  let host: string;
  try { host = new URL(target).hostname; } catch { host = target; }
  // Step 1: Load tool registry and group tools
  const __filename = fileURLToPath(import.meta.url);
  const __dirname = dirname(__filename);
  // Try CWD first, then fallback to project root tools.yaml
  const cwdPath = path.resolve(process.cwd(), 'tools.yaml');
  const modulePath = path.resolve(__dirname, '../../tools.yaml');
  let toolsPath: string;
  if (fs.existsSync(cwdPath)) toolsPath = cwdPath;
  else if (fs.existsSync(modulePath)) toolsPath = modulePath;
  else {
    console.error(`tools.yaml not found at ${cwdPath} or ${modulePath}`);
    toolsPath = cwdPath; // will error when read
  }
  let registry: any = {};
  try {
    const raw = fs.readFileSync(toolsPath, 'utf8');
    registry = yaml.load(raw) as any;
  } catch (err) {
    console.error(`Failed to load tools.yaml at ${toolsPath}: ${err}`);
  }
  const reconTools: Array<any> = [];
  const exploitTools: Array<any> = [];
  const toolInfoByName: Record<string, any> = {};
  if (registry.tools) {
    for (const [name, info] of Object.entries(registry.tools)) {
      const entry = {
        name,
        command: info.command,
        parser: info.parser,
        retries: info.retries ?? 1,
        timeout: info.timeout ?? 300,
        outputSchema: info.output_schema || null,
        followup: info.followup?.when_successful || []
      };
      toolInfoByName[name] = entry;
      if (info.type === 'recon') reconTools.push(entry);
      if (info.type === 'exploit') exploitTools.push(entry);
    }
  }
  // Debug: show tool groups if requested
  if (process.argv.includes('--debug-tools')) {
    console.log('Recon tools:', reconTools);
    console.log('Exploit tools:', exploitTools);
    return;
  }

  // Parser dispatcher
  async function parseOutput(raw: string, parser?: string): Promise<any> {
    if (parser === 'json') {
      return JSON.parse(raw);
    } else if (parser === 'json_lines') {
      return raw
        .split(/\r?\n/)
        .map(l => l.trim())
        .filter(l => l)
        .map(l => JSON.parse(l));
    } else if (parser === 'xml') {
      return await parseStringPromise(raw);
    }
    // plain or undefined
    return raw;
  }
  // 2) Spawn a pseudo-terminal for command execution
  const shell = process.env.SHELL || (os.platform() === 'win32' ? 'powershell.exe' : 'bash');
  const ptyProc = pty.spawn(shell, ['-i'], { name: 'xterm-color', cwd: process.cwd(), env: process.env });
  ptyProc.on('data', data => process.stdout.write(data));

  // 2) Ensure tools directory and inventory
  await detectTools();
  // Initialize structured state
  const state: any = {
    target,
    created_at: new Date().toISOString(),
    steps: []
  };
  const stateFile = 'state.yaml';
  const flushState = () => fs.writeFileSync(stateFile, yaml.dump(state));
  // Immediately write initial state
  flushState();

  // 3) Initialize LLM and parse mission scope
  const openai = new OpenAI({ apiKey: config.apiKey });
  let messages: Array<{ role: string; content: string }> = [
    { role: 'system', content: `You are Adversys Predator Mode. Execute full exploit chain autonomously.` },
    { role: 'system', content: `Target: ${target}` }
  ];
  // Include user-provided images
  if (options.imagePaths && options.imagePaths.length > 0) {
    for (const imgPath of options.imagePaths) {
      if (fs.existsSync(imgPath)) {
        messages.push({ role: 'user', content: `Image provided: ${imgPath}` });
      } else {
        console.warn(`[WARN] Image not found: ${imgPath}`);
      }
    }
  }
  // Map mission flag to OWASP phase names
  const MISSION_MAP: Record<string,string> = {
    'owasp-a01': 'Broken Access Control', 'a01': 'Broken Access Control', 'broken access control': 'Broken Access Control',
    'owasp-a02': 'Cryptographic Failures', 'a02': 'Cryptographic Failures', 'cryptographic failures': 'Cryptographic Failures',
    'owasp-a03': 'Injection',           'a03': 'Injection',           'injection': 'Injection',
    'owasp-a04': 'Insecure Design',      'a04': 'Insecure Design',      'insecure design': 'Insecure Design',
    'owasp-a05': 'Security Misconfiguration','a05': 'Security Misconfiguration','security misconfiguration': 'Security Misconfiguration',
    'owasp-a06': 'Vulnerable and Outdated Components','a06': 'Vulnerable and Outdated Components','vulnerable and outdated components': 'Vulnerable and Outdated Components',
    'owasp-a07': 'Identification and Authentication Failures','a07':'Identification and Authentication Failures','identification and authentication failures':'Identification and Authentication Failures',
    'owasp-a08': 'Software and Data Integrity Failures','a08':'Software and Data Integrity Failures','software and data integrity failures':'Software and Data Integrity Failures',
    'owasp-a09': 'Security Logging and Monitoring Failures','a09':'Security Logging and Monitoring Failures','security logging and monitoring failures':'Security Logging and Monitoring Failures',
    'owasp-a10': 'Server-Side Request Forgery (SSRF)','a10':'Server-Side Request Forgery (SSRF)','server-side request forgery (ssrf)':'Server-Side Request Forgery (SSRF)',
    'all': 'all'
  };
  const missionInput = options.mission?.toLowerCase();
  let phases = OWASP_TOP10;
  let missionScope = 'all';
  if (missionInput && missionInput !== 'all') {
    missionScope = MISSION_MAP[missionInput] || missionInput;
    if (MISSION_MAP[missionInput] || OWASP_TOP10.includes(missionScope)) {
      phases = [missionScope];
    } else {
      console.warn(`Unknown mission scope '${options.mission}', running full OWASP Top 10`);
      missionScope = 'all';
    }
  }
  console.log(`🎯 Running mission scope: ${missionScope}${missionScope !== 'all' ? ` (${missionInput})` : ''}`);
  state.mission_scope = missionScope;
  state.skipped_phases = OWASP_TOP10.filter(p => !phases.includes(p));
  flushState();

  // 4) Iterate through OWASP Top 10
  for (const vuln of phases) {
    console.log(`\n[Phase - Detect] ${vuln}`);
    // Ask LLM for detection command
    let detectJson = '';
    try {
      // Spinner while LLM plans detection
      const stopSpinner = startSpinner(`Detecting ${vuln}`);
      const plan = await openai.chat.completions.create({
        model: config.model,
        messages: [
          ...messages,
          { role: 'user', content: `Generate a JSON with keys 'tool' and 'args' to detect ${vuln} on target '${target}'.` }
        ]
      });
      stopSpinner();
      detectJson = plan.choices?.[0]?.message?.content ?? '';
    } catch (e: any) {
      console.error('LLM detection planning error:', e);
      continue;
    }
    let detectCmd = { tool: '', args: '' };
    // Attempt LLM-suggested detection command
    let detected = false;
    try {
      detectCmd = JSON.parse(detectJson.replace(/```(?:json)?/g, '').replace(/```/g, '').trim());
      if (detectCmd.tool && detectCmd.args) detected = true;
      else console.error('LLM suggested empty tool or args');
    } catch (e: any) {
      console.error('Failed to parse detection JSON:', e.message || detectJson);
      console.error('Failed to parse detection JSON:', detectJson);
    }
    // Fallback to static recon tools if LLM failed
    if (!detected) {
      for (const fb of reconTools) {
        // Spinner for fallback detection
        const stopSpinnerFB = startSpinner(`Fallback detect: ${fb.name}`);
        console.log(`Fallback detect: ${fb.name}`);
        let out = '';
        const onData = (d: string) => out += d;
        ptyProc.on('data', onData);
        // substitute both host and target
        const fbCmd = fb.command.replace('{host}', host).replace('{target}', normalizedTarget);
        ptyProc.write(fbCmd + '\r');
        await new Promise(r => setTimeout(r, fb.timeout * 1000));
        ptyProc.removeListener('data', onData);
        stopSpinnerFB();
        if (out.trim()) {
          console.log(`Detected via fallback ${fb.name}`);
          recordCommand(options.session!, `Detect:${vuln}`, out, 0);
          messages.push({ role: 'assistant', content: out });
          detected = true;
          break;
        }
      }
      if (!detected) continue;
      detectCmd = { tool: '', args: '' }; // clear for next use
    }
    // Install tool if needed
    if (!isToolInstalled(detectCmd.tool)) {
      console.log(`Installing tool: ${detectCmd.tool}`);
      try {
        await installTool(detectCmd.tool);
      } catch (err: any) {
        console.error(`[ERROR] Failed to install tool '${detectCmd.tool}':`, err.message || err);
        continue;
      }
    }
    // Execute detection
    console.log(`Detect command: ${detectCmd.tool} ${detectCmd.args}`);
    ptyProc.write(`${detectCmd.tool} ${detectCmd.args}\r`);
    let output = '';
    const onData = (d: string) => output += d;
    ptyProc.on('data', onData);
    await new Promise(r => setTimeout(r, 3000));
    ptyProc.removeListener('data', onData);
    recordCommand(options.session!, `Detect:${vuln}`, output, 0);
    messages.push({ role: 'assistant', content: output });
    // Parse output
    let parsed: any = null;
    let parseError: string | null = null;
    try {
      parsed = await parseOutput(output, toolInfoByName[detectCmd.tool]?.parser);
    } catch (e: any) {
      parseError = e.message;
    }
    // Record structured step
    state.steps.push({
      phase: vuln,
      stage: 'detect',
      tool: detectCmd.tool,
      args: detectCmd.args,
      timestamp: new Date().toISOString(),
      success: parseError === null,
      output: output.trim(),
      parsedOutput: parsed,
      error: parseError
    });
    flushState();

    console.log(`\n[Phase - Exploit] ${vuln}`);
    // Ask LLM for exploit command
    let exploitJson = '';
    try {
      // Spinner while LLM plans exploit
      const stopSpinner = startSpinner(`Exploiting ${vuln}`);
      const plan2 = await openai.chat.completions.create({
        model: config.model,
        messages: [
          ...messages,
          { role: 'user', content: `Generate a JSON with keys 'tool' and 'args' to exploit ${vuln} on target '${target}'.` }
        ]
      });
      stopSpinner();
      exploitJson = plan2.choices?.[0]?.message?.content ?? '';
    } catch (e: any) {
      console.error('LLM exploit planning error:', e);
      continue;
    }
    let exploitCmd = { tool: '', args: '' };
    // Attempt LLM-suggested exploit
    let exploited = false;
    try {
      // strip code fences and parse
      const cleaned = exploitJson.replace(/```(?:json)?/g, '').replace(/```/g, '').trim();
      exploitCmd = JSON.parse(cleaned);
      if (exploitCmd.tool && exploitCmd.args) exploited = true;
      else console.error('LLM suggested empty exploit tool or args');
    } catch (e: any) {
      console.error('Failed to parse exploit JSON:', e.message || exploitJson);
    }
    // Fallback to static exploit tools if LLM failed
    if (!exploited) {
      for (const fb of exploitTools) {
        // Spinner for fallback exploit
        const stopSpinnerEX = startSpinner(`Fallback exploit: ${fb.name}`);
        console.log(`Fallback exploit: ${fb.name}`);
        let out = '';
        const onData2 = (d: string) => out += d;
        ptyProc.on('data', onData2);
        const fbCmd = fb.command.replace('{host}', host).replace('{target}', normalizedTarget);
        ptyProc.write(fbCmd + '\r');
        await new Promise(r => setTimeout(r, fb.timeout * 1000));
        ptyProc.removeListener('data', onData2);
        stopSpinnerEX();
        if (out.trim()) {
          console.log(`Exploited via fallback ${fb.name}`);
          recordCommand(options.session!, `Exploit:${vuln}`, out, 0);
          messages.push({ role: 'assistant', content: out });
          exploited = true;
          break;
        }
      }
      if (!exploited) continue;
    }
    // Install exploit tool if needed
    if (!isToolInstalled(exploitCmd.tool)) {
      if (options.noInstall) {
        console.log(`[WARN] Tool '${exploitCmd.tool}' not installed; skipping auto-install (--no-install)`);
      } else {
        console.log(`Installing tool: ${exploitCmd.tool}`);
        try {
          await installTool(exploitCmd.tool);
        } catch (err: any) {
          console.error(`[ERROR] Failed to install tool '${exploitCmd.tool}':`, err.message || err);
          continue;
        }
      }
    }
    console.log(`Exploit command: ${exploitCmd.tool} ${exploitCmd.args}`);
    ptyProc.write(`${exploitCmd.tool} ${exploitCmd.args}\r`);
    let exploitOut = '';
    const onData2 = (d: string) => exploitOut += d;
    ptyProc.on('data', onData2);
    await new Promise(r => setTimeout(r, 3000));
    ptyProc.removeListener('data', onData2);
    recordCommand(options.session!, `Exploit:${vuln}`, exploitOut, 0);
    messages.push({ role: 'assistant', content: exploitOut });
    // Parse exploit output
    let parsedExploit: any = null;
    let exploitError: string | null = null;
    try {
      parsedExploit = await parseOutput(exploitOut, toolInfoByName[exploitCmd.tool]?.parser);
    } catch (e: any) {
      exploitError = e.message;
    }
    // Record structured exploit step
    state.steps.push({
      phase: vuln,
      stage: 'exploit',
      tool: exploitCmd.tool,
      args: exploitCmd.args,
      timestamp: new Date().toISOString(),
      success: exploitError === null,
      output: exploitOut.trim(),
      parsedOutput: parsedExploit,
      error: exploitError
    });
    flushState();
    // 5) Post-exploitation chaining
    const followupList: string[] = toolInfoByName[exploitCmd.tool]?.followup || [];
    for (const nextTool of followupList) {
      const info = toolInfoByName[nextTool];
      if (!info) {
        console.warn(`Unknown follow-up tool: ${nextTool}`);
        continue;
      }
      console.log(`\n[Phase - Followup] ${vuln} -> ${nextTool}`);
      // Install follow-up tool if needed
      if (!isToolInstalled(info.name)) {
        if (options.noInstall) {
          console.log(`[WARN] Follow-up tool '${info.name}' not installed; skipping auto-install (--no-install)`);
        } else {
          console.log(`Installing follow-up tool: ${info.name}`);
          try {
            await installTool(info.name);
          } catch (err: any) {
            console.error(`[ERROR] Failed to install follow-up tool '${info.name}':`, err.message || err);
            continue;
          }
        }
      }
      // Execute follow-up
      const fuRawCmd = info.command.replace('{target}', normalizedTarget);
      console.log(`Follow-up command: ${fuRawCmd}`);
      const stopSpinnerFU = startSpinner(`Followup ${nextTool}`);
      let fuOut = '';
      const onFuData = (d: string) => fuOut += d;
      ptyProc.on('data', onFuData);
      ptyProc.write(fuRawCmd + '\r');
      await new Promise(r => setTimeout(r, info.timeout * 1000));
      ptyProc.removeListener('data', onFuData);
      stopSpinnerFU();
      // Parse follow-up output
      let fuParsed = null;
      let fuError: string | null = null;
      try { fuParsed = await parseOutput(fuOut, info.parser); } catch (e: any) { fuError = e.message; }
      // Record follow-up step
      state.steps.push({
        phase: vuln,
        stage: 'followup',
        tool: info.name,
        args: fuRawCmd,
        timestamp: new Date().toISOString(),
        success: fuError === null,
        output: fuOut.trim(),
        parsedOutput: fuParsed,
        error: fuError
      });
      flushState();
    }
  }
  console.log('\n🎯 Predator mission completed across OWASP Top 10.');
}