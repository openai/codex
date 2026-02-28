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
const schemaRootRel = `artifacts/pr-packets/${runId}/generated-schema`;
const schemaRootAbs = path.join(repoRoot, ...schemaRootRel.split('/'));
const generatedPath = path.join(schemaRootAbs, 'json', 'codex_app_server_protocol.schemas.json');
const command = `cargo run -p codex-app-server-protocol --bin write_schema_fixtures -- --schema-root ${schemaRootRel}`;
const diffPathRel = `artifacts/pr-packets/${runId}/contract-check.diff.txt`;
const start = Date.now();

const result = {
  runId,
  status: 'ERROR',
  command,
  expectedHash: '',
  generatedHash: '',
  exitCode: 2,
  durationMs: 0,
  diffPath: diffPathRel,
  timestamp: new Date().toISOString(),
};

const writeOutput = (diffText) => {
  mkdirSync(packetDir, { recursive: true });
  const checkOutput = `${JSON.stringify(result, null, 2)}\n`;
  writeFileSync(path.join(packetDir, 'contract-check.json'), checkOutput);
  writeFileSync(path.join(packetDir, 'contract-check.diff.txt'), diffText ?? '');
};

const finalizeAndExit = (status, exitCode, diffText) => {
  result.status = status;
  result.exitCode = exitCode;
  result.durationMs = Date.now() - start;
  result.timestamp = new Date().toISOString();
  writeOutput(diffText);
  process.exit(exitCode);
};

const errorExit = (message) => {
  const output = `${message.trim()}\n`;
  console.error(output);
  finalizeAndExit('ERROR', 2, output);
};

try {
  if (!existsSync(contractExpected)) {
    errorExit(`Expected contract file missing at ${contractExpected}`);
  }

  const expected = readFileSync(contractExpected);
  mkdirSync(schemaRootAbs, { recursive: true });

  const generate = spawnSync(
    'cargo',
    ['run', '-p', 'codex-app-server-protocol', '--bin', 'write_schema_fixtures', '--', '--schema-root', schemaRootAbs],
    {
      cwd: path.join(repoRoot, 'codex-rs'),
      encoding: 'utf8',
      shell: false,
    }
  );

  if (generate.error || generate.status !== 0) {
    const output = [generate.stdout, generate.stderr].filter(Boolean).join('\n').trim();
    const msg = [
      'Contract generation command failed.',
      `Command: ${command}`,
      `Exit code: ${generate.status ?? 'null'}`,
      output,
    ]
      .filter(Boolean)
      .join('\n');
    errorExit(msg);
  }

  if (!existsSync(generatedPath)) {
    errorExit(`Generated schema path not found at ${generatedPath}`);
  }

  const generated = readFileSync(generatedPath);

  result.expectedHash = createHash('sha256').update(expected).digest('hex');
  result.generatedHash = createHash('sha256').update(generated).digest('hex');

  const diffProc = spawnSync(
    'git',
    ['diff', '--no-index', '--', contractExpected, generatedPath],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      shell: false,
    }
  );

  const mismatch = result.expectedHash !== result.generatedHash;
  let diffText = `${diffProc.stdout || ''}${diffProc.stderr || ''}`;

  if (diffProc.error || (diffProc.status ?? 0) > 1) {
    if (mismatch) {
      diffText = [
        'Contract drift detected, but git diff failed.',
        `Expected file: ${contractExpected}`,
        `Generated file: ${generatedPath}`,
        `Expected hash: ${result.expectedHash}`,
        `Generated hash: ${result.generatedHash}`,
      ].join('\n');
    } else {
      diffText = 'No contract drift detected.\n';
    }
  }

  if (mismatch && diffText.trim() === '') {
    diffText = [
      'Contract drift detected.',
      `Expected file: ${contractExpected}`,
      `Generated file: ${generatedPath}`,
      `Expected hash: ${result.expectedHash}`,
      `Generated hash: ${result.generatedHash}`,
    ].join('\n');
  }

  if (!mismatch && diffText.trim() === '') {
    diffText = 'No contract drift detected.\n';
  }

  finalizeAndExit(mismatch ? 'FAIL' : 'PASS', mismatch ? 1 : 0, diffText);
} catch (err) {
  const msg = `Runtime error during contract check: ${err instanceof Error ? err.message : String(err)}`;
  errorExit(msg);
}
