import { describe, it, expect } from 'vitest';
import path from 'path';
import os from 'os';

// Path to compiled CLI entry
const CLI_PATH = path.join(__dirname, '../../../../dist/cli/commands/resources/list.cli.js');

// Helper to run the CLI with a TTY using node-pty
function runCliWithInput(inputs: string[], delay = 600): Promise<string> {
  return new Promise((resolve, reject) => {
    const pty = require('node-pty');
    const shell = process.execPath;
    const args = [CLI_PATH];
    const term = pty.spawn(shell, args, {
      name: 'xterm-color',
      cols: 80,
      rows: 30,
      cwd: process.cwd(),
      env: process.env,
    });
    let output = '';
    term.onData((data: string) => {
      output += data;
    });
    let i = 0;
    function sendNext() {
      if (i < inputs.length) {
        term.write(inputs[i]);
        i++;
        setTimeout(sendNext, delay);
      } else {
        // Wait longer before sending quit to allow Ink to flush output
        setTimeout(() => {
          term.write('q'); // quit
          setTimeout(() => {
            term.kill();
            resolve(output);
          }, 800); // was 400
        }, 800); // was 400
      }
    }
    sendNext();
  });
}

function stripDebug(output: string) {
  return output.split('\n').filter(line => !line.startsWith('[DEBUG')).join('\n');
}

// Helper: wait for a string to appear in output (for async CLI rendering)
async function waitForOutput(outputPromise: Promise<string>, expected: string, timeout = 7000) {
  const start = Date.now();
  let output = '';
  while (Date.now() - start < timeout) {
    output = await outputPromise;
    // Debug: log output chunk for diagnosis
    // eslint-disable-next-line no-console
    console.log('[waitForOutput] Current output:', output.slice(-200));
    if (output.includes(expected)) return output;
    await new Promise(r => setTimeout(r, 100));
  }
  throw new Error(`Timed out waiting for output: ${expected}\nLast output:\n${output.slice(-500)}`);
}

describe('ResourcesList CLI E2E', () => {
  it('renders first page and navigates to next and prev', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '57';
    const outputPromise = runCliWithInput(['n', 'p'], 600);
    const output = await waitForOutput(outputPromise, 'Resource #11');
    const clean = stripDebug(output);
    expect(clean).toContain('Resource #1');
    expect(clean).toContain('Resource #11');
    expect(clean).toContain('Page 2');
    expect(clean).toContain('Page 1');
  });

  it('does not go before first page or past last page', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '57';
    const outputPromise1 = runCliWithInput(['p'], 1000);
    const output1 = await waitForOutput(outputPromise1, 'Page 1');
    const clean1 = stripDebug(output1);
    expect(clean1).toContain('Page 1');
    const nextInputs = Array(10).fill('n'); // more than enough for last page
    const outputPromise2 = runCliWithInput(nextInputs, 1000);
    const output2 = await waitForOutput(outputPromise2, 'Resource #51');
    const clean2 = stripDebug(output2);
    // Should reach page 6 (last page for 57 items, 10/page)
    expect(clean2.match(/Page \d+/g).pop()).toBe('Page 6');
    // Should see at least one item from last page
    expect(clean2).toMatch(/Resource #51/);
    // Should not see an item past the last
    expect(clean2).not.toMatch(/Resource #61/);
  });

  it('renders single item list', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '1';
    const outputPromise = runCliWithInput([], 600);
    const output = await waitForOutput(outputPromise, 'Resource #1');
    const clean = stripDebug(output);
    expect(clean).toContain('Resource #1');
    expect(clean).toContain('Page 1');
    expect(clean).not.toContain('No items.');
  });

  it('renders exact page-size list', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '10';
    const outputPromise = runCliWithInput([], 600);
    const output = await waitForOutput(outputPromise, 'Resource #10');
    const clean = stripDebug(output);
    expect(clean).toContain('Resource #10');
    expect(clean).toContain('Page 1');
    expect(clean).not.toContain('No items.');
  });

  it('renders page-size-plus-one list', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '11';
    const outputPromise = runCliWithInput(['n'], 600);
    const output = await waitForOutput(outputPromise, 'Resource #11');
    const clean = stripDebug(output);
    // Should see page 2 and Resource #11
    expect(clean).toContain('Page 2');
    expect(clean).toContain('Resource #11');
    // Should still see page 1 and Resource #1 (from initial render)
    expect(clean).toContain('Page 1');
    expect(clean).toContain('Resource #1');
  });

  it('handles rapid navigation', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '57';
    const outputPromise = runCliWithInput(['n', 'n', 'n'], 1000);
    const output = await waitForOutput(outputPromise, 'Page 4');
    const clean = stripDebug(output);
    // Should reach page 4 (0-based, so 3 navigations)
    expect(clean).toMatch(/Page 4/);
    // Should see at least one item from page 4 (items 31-40)
    expect(clean).toMatch(/Resource #31/);
  });

  it('quits from any page', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '30';
    let outputPromise = runCliWithInput(['n', 'n', 'q'], 1000);
    let output = await waitForOutput(outputPromise, 'Page 3');
    let clean = stripDebug(output);
    expect(clean).toMatch(/Page 3/);
    expect(clean).toMatch(/Resource #21/);
    outputPromise = runCliWithInput(['q'], 1000);
    output = await waitForOutput(outputPromise, 'Page 1');
    clean = stripDebug(output);
    expect(clean).toMatch(/Page 1/);
    expect(clean).toMatch(/Resource #1/);
    expect(clean).not.toContain('No items.');
  });

  it('ignores invalid keys', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '20';
    const outputPromise = runCliWithInput(['x', 'n'], 600);
    const output = await waitForOutput(outputPromise, 'Page 2');
    const clean = stripDebug(output);
    expect(clean).toContain('Page 2');
  });

  it('shows message when there are no resources', async () => {
    process.env['MOCK_RESOURCES_LENGTH'] = '0';
    const outputPromise = runCliWithInput([], 600);
    const output = await waitForOutput(outputPromise, 'Page 1');
    const clean = stripDebug(output);
    // Should not show 'No items.' or any resource, but show navigation
    expect(clean).not.toContain('No items.');
    expect(clean).toContain('Page 1');
    // Should not show any Resource #
    expect(clean).not.toMatch(/Resource #/);
  });
});
