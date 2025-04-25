import fs from "fs";
import net from "net";
import os from "os";
import pty from "node-pty";

/**
 * Runs the shell-agent process: listens on a Unix socket for run commands,
 * executes them in a PTY, and streams back output and exit codes.
 * @param ipcSocket Path to the Unix domain socket
 */
export async function runShellAgent(ipcSocket: string): Promise<void> {
  if (!ipcSocket) {
    console.error("Error: --ipc-socket <path> is required");
    process.exit(1);
  }
  // Remove stale socket
  try { fs.unlinkSync(ipcSocket); } catch {}
  // Spawn a shell PTY
  const shell = process.env.SHELL || (os.platform() === 'win32' ? 'powershell.exe' : 'bash');
  const ptyProc = pty.spawn(shell, ['-i'], {
    name: 'xterm-color',
    cwd: process.cwd(),
    env: process.env,
  });
  // Create Unix socket server
  const server = net.createServer((socket) => {
    socket.setEncoding('utf8');
    // Relay PTY output to socket
    ptyProc.on('data', (data: string) => {
      const msg = { type: 'output', data };
      socket.write(JSON.stringify(msg) + '\n');
    });
    // On PTY exit, send exit notification
    ptyProc.on('exit', (code: number) => {
      const msg = { type: 'exit', exitCode: code };
      socket.write(JSON.stringify(msg) + '\n');
    });
    // Handle incoming run requests
    let buffer = '';
    socket.on('data', (chunk: string) => {
      buffer += chunk;
      let idx: number;
      while ((idx = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, idx).trim();
        buffer = buffer.slice(idx + 1);
        if (!line) continue;
        try {
          const msg = JSON.parse(line);
          console.log("[DEBUG][Agent] Received IPC message:", msg);
          if (msg.type === 'run' && typeof msg.cmd === 'string') {
            console.log("[DEBUG][Agent] Writing to PTY:", msg.cmd);
            // send command to PTY with newline
            ptyProc.write(msg.cmd + '\r\n');
          }
        } catch (e) {
          console.error('[ERROR][Agent] Malformed IPC JSON:', line);
        }
      }
    });
    socket.on('error', (err) => console.error('[ERROR][Agent] IPC socket error:', err));
  });
  server.listen(ipcSocket, () => {
    console.log(`shell-agent listening on ${ipcSocket}`);
  });
  server.on('error', (err) => {
    console.error('[ERROR][Agent] IPC server error:', err);
    process.exit(1);
  });
  // Block forever to keep server alive
  await new Promise(() => {});
}