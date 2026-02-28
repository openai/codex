#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const args = process.argv.slice(2);
const getArg = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  if (index === -1) {
    return fallback;
  }
  return args[index + 1] ?? fallback;
};

const runId = getArg('run-id', `run-${new Date().toISOString().replace(/[:.]/g, '-')}`);
const repoRoot = process.cwd();
const packetDir = path.join(repoRoot, 'artifacts', 'pr-packets', runId);
const contractExpected = path.join(repoRoot, 'contracts', 'app-schema.expected.json');
const generatedRoot = path.join(repoRoot, 'artifacts', 'tmp-schema', runId);
const command = ['cargo run -p codex-app-server-protocol --bin write_schema_fixtures -- --schema-root', generatedRoot];
let diffText = '';

const result = {
  runId,
  status: 'FAIL',
  command: command.join(' '),
  expectedHash: '',
  generatedHash: '',
  exitCode: 1,
  durationMs: 0,
  diffPath: path.join('artifacts', 'pr-packets', runId, 'contract-check.diff.txt'),
  timestamp: new Date().toISOString(),
};

const writeFailure = (message, exitCode = 2) => {
  result.exitCode = exitCode;
  result.status = 'ERROR';
  result.timestamp = new Date().toISOString();
  writeOutput();
  console.error(message);
  process.exit(exitCode);
};

const writeOutput = () => {
  mkdirSync(packetDir, { recursive: true });
  const checkOutput = `${JSON.stringify(result, null, 2)}\n`;
  writeFileSync(path.join(packetDir, 'contract-check.json'), checkOutput);
  writeFileSync(path.join(packetDir, 'contract-check.diff.txt'), diffText);
};

const expected = existsSync(contractExpected) ? readFileSync(contractExpected) : null;
if (!expected) {
  writeFailure(`Expected contract file missing at ${contractExpected}`, 2);
}

const start = Date.now();
try {
  mkdirSync(generatedRoot, { recursive: true });
  const generate = spawnSync(
    'cargo',
    ['run', '-p', 'codex-app-server-protocol', '--bin', 'write_schema_fixtures', '--', '--schema-root', generatedRoot],
    {
      cwd: path.join(repoRoot, 'codex-rs'),
      encoding: 'utf8',
      shell: false,
    }
  );

  const generatedPath = path.join(generatedRoot, 'json', 'codex_app_server_protocol.schemas.json');

  if (generate.status !== 0) {
    result.exitCode = 2;
    result.status = 'ERROR';
    result.durationMs = Date.now() - start;
    const diffErr = [generate.stdout, generate.stderr].filter(Boolean).join('\n');
    const msg = [
      'Contract generation command failed.',
      diffErr,
      `Exit code: ${generate.status}`,
    ]
      .filter(Boolean)
      .join('\n');

    result.commandOutput = msg;
    writeFailure(msg, 2);
  }

  if (!existsSync(generatedPath)) {
    writeFailure(`Generated schema path not found at ${generatedPath}`);
  }

  const generated = readFileSync(generatedPath);

  result.expectedHash = createHash('sha256').update(expected).digest('hex');
  result.generatedHash = createHash('sha256').update(generated).digest('hex');

  const diff = spawnSync(
    'git',
    ['diff', '--no-index', '--', contractExpected, generatedPath],
    {
      encoding: 'utf8',
      shell: false,
    }
  );
  diffText = (diff.stdout || '') + (diff.stderr || '');

  const mismatch = result.expectedHash !== result.generatedHash;
  result.status = mismatch ? 'FAIL' : 'PASS';
  result.exitCode = mismatch ? 1 : 0;
  result.durationMs = Date.now() - start;

  writeOutput();

  if (mismatch) {
    process.exit(1);
  }

  process.exit(0);
} catch (err) {
  const msg = `Runtime error during contract check: ${err instanceof Error ? err.message : String(err)}`;
  writeFailure(msg, 2);
}
