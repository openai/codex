#!/usr/bin/env node

import { promises as fs } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

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
const logPath = path.join(packetDir, 'test-logs.txt');

const coordinationDir = path.join(repoRoot, 'artifacts', 'coordination', runId);
const watchScopePaths = {
  contractCheckPath: path.join(packetDir, 'contract-check.json'),
  contractDiffPath: path.join(packetDir, 'contract-check.diff.txt'),
};

let testLog = '';

const runCommand = (cmd, args, cwd = repoRoot, label = `${cmd} ${args.join(' ')}`) => {
  testLog += `=== ${label}\n`;
  const proc = spawnSync(cmd, args, {
    cwd,
    encoding: 'utf8',
    shell: false,
  });

  if (proc.stdout) {
    testLog += proc.stdout;
  }
  if (proc.stderr) {
    testLog += proc.stderr;
  }
  testLog += `\nExitCode: ${proc.status}\n\n`;

  return {
    label,
    status: proc.status === 0 ? 'PASS' : 'FAIL',
    code: proc.status ?? -1,
  };
};

const writeFile = async (target, content) => {
  await fs.mkdir(path.dirname(target), { recursive: true });
  await fs.writeFile(target, content, 'utf8');
};

const main = async () => {
  await fs.mkdir(packetDir, { recursive: true });

  const commands = [runCommand('cargo', ['test', '-p', 'codex-app-server-protocol', 'schema_fixtures_match_generated'], path.join(repoRoot, 'codex-rs'))];

  // Capture git patch from current branch integration state.
  const gitDiff = spawnSync('git', ['diff', '--binary'], {
    cwd: repoRoot,
    encoding: 'utf8',
    shell: false,
  });
  await writeFile(path.join(packetDir, 'diff.patch'), gitDiff.stdout || '');

  const contractCheck = await (async () => {
    try {
      const raw = await fs.readFile(watchScopePaths.contractCheckPath, 'utf8');
      return JSON.parse(raw);
    } catch {
      return {
        status: 'MISSING',
        command: '',
        expectedHash: '',
        generatedHash: '',
        exitCode: 2,
        diffPath: path.join('artifacts', 'pr-packets', runId, 'contract-check.diff.txt'),
        timestamp: new Date().toISOString(),
      };
    }
  })();

  const blockContract = contractCheck.exitCode !== 0 || contractCheck.status !== 'PASS';

  if (blockContract) {
    testLog += 'Contract gate did not pass. Integration blocked by design.\n';
  } else {
    testLog += 'Contract gate passed.\n';
  }

  await writeFile(logPath, testLog);

  // Pass through watcher artifacts.
  const contractDiffSource = watchScopePaths.contractDiffPath;
  const contractJsonSource = watchScopePaths.contractCheckPath;
  try {
    await writeFile(path.join(packetDir, 'contract-check.diff.txt'), await fs.readFile(contractDiffSource, 'utf8'));
  } catch {
    await writeFile(path.join(packetDir, 'contract-check.diff.txt'), '');
  }

  try {
    await writeFile(path.join(packetDir, 'contract-check.json'), await fs.readFile(contractJsonSource, 'utf8'));
  } catch {
    await writeFile(path.join(packetDir, 'contract-check.json'), JSON.stringify(contractCheck, null, 2));
  }

  // Impact report
  const impactReport = {
    runId,
    generatedAt: new Date().toISOString(),
    status: blockContract ? 'BLOCKED' : 'READY_TO_MERGE',
    checks: {
      protocolFixture: {
        status: commands[0].status,
        command: commands[0].label,
      },
      baselineSchemaRefresh: {
        status: commands[1].status,
        command: commands[1].label,
      },
      contractGate: {
        status: contractCheck.status,
        command: contractCheck.command,
        exitCode: contractCheck.exitCode,
      },
    },
    coordination: {
      runId,
      files: [],
    },
  };

  const coordinationEntries = await fs.readdir(coordinationDir).catch(() => []);
  for (const file of coordinationEntries) {
    const full = path.join(coordinationDir, file);
    if (file.endsWith('.json')) {
      const stat = await fs.lstat(full);
      if (stat.isFile()) {
        try {
          impactReport.coordination.files.push({
            file,
            contents: JSON.parse(await fs.readFile(full, 'utf8')),
          });
        } catch {
          impactReport.coordination.files.push({
            file,
            contents: `Could not parse ${file}`,
          });
        }
      }
    }
  }

  await writeFile(path.join(packetDir, 'impact-report.json'), `${JSON.stringify(impactReport, null, 2)}\n`);

  const summaryLines = [
    '# PR Packet Summary',
    '',
    `Run ID: ${runId}`,
    '',
    `Contract Gate: ${impactReport.status}`,
    '',
    '## Stage Matrix',
    `- Protocol fixture test: ${commands[0].status} (${commands[0].code})`,
    `- Contract check: ${contractCheck.status} (${contractCheck.exitCode})`,
    '',
    '## Files',
    `- artifacts/pr-packets/${runId}/diff.patch`,
    `- artifacts/pr-packets/${runId}/test-logs.txt`,
    `- artifacts/pr-packets/${runId}/contract-check.json`,
    `- artifacts/pr-packets/${runId}/contract-check.diff.txt`,
    `- artifacts/pr-packets/${runId}/impact-report.json`,
    `- artifacts/pr-packets/${runId}/summary.md`,
    '',
  ];

  if (blockContract) {
    summaryLines.push('## Blocker');
    summaryLines.push('- Contract drift detected; do not merge until `app-schema.expected.json` is updated by implementer B.');
    summaryLines.push('- Retry contract check after follow-up commit.');
  } else {
    summaryLines.push('## Next Step');
    summaryLines.push('- CONTRACT CHECK PASSED. PR packet is ready to merge.');
  }

  await writeFile(path.join(packetDir, 'summary.md'), `${summaryLines.join('\n')}\n`);

  if (blockContract) {
    process.exit(1);
  }
  process.exit(0);
};

main().catch(async (err) => {
  testLog += `Fatal error while generating packet: ${String(err)}\n`;
  await writeFile(logPath, testLog);
  process.exit(2);
});
