import os from "os";
import { spawn } from "child_process";
import { readFileSync } from "fs";
import { detectTools } from "../tools/detection";

/**
 * Install a missing security tool via the system package manager and refresh detection.
 * Supports macOS (brew) and common Linux distros (apt, dnf, yum, pacman, apk, zypper).
 * Throws on unsupported platforms or install failures.
 */
export async function installTool(name: string): Promise<void> {
  let installCmd: string;
  switch (os.platform()) {
    case "darwin":
      installCmd = `brew install ${name}`;
      break;
    case "linux": {
      // Detect distro via /etc/os-release
      const osRelease = readFileSync('/etc/os-release', 'utf8');
      const data: Record<string,string> = {};
      for (const line of osRelease.split(/\r?\n/)) {
        const kv = line.match(/^([A-Z_]+)=(?:"?)(.+?)(?:"?)$/);
        if (kv) data[kv[1]] = kv[2];
      }
      const id = (data['ID'] || '').toLowerCase();
      const idLike = (data['ID_LIKE'] || '').toLowerCase().split(/\s+/);
      // Choose package manager based on distro
      if (id === 'ubuntu' || id === 'debian' || id === 'kali' || idLike.includes('debian')) {
        installCmd = `sudo apt-get update && sudo apt-get install -y ${name}`;
      } else if (id === 'fedora' || idLike.includes('fedora')) {
        installCmd = `sudo dnf install -y ${name}`;
      } else if (id === 'centos' || id === 'rhel' || idLike.includes('rhel') || idLike.includes('centos')) {
        installCmd = `sudo yum install -y ${name}`;
      } else if (id === 'arch' || idLike.includes('arch')) {
        installCmd = `sudo pacman -S --noconfirm ${name}`;
      } else if (id === 'alpine' || idLike.includes('alpine')) {
        installCmd = `sudo apk add ${name}`;
      } else if (id === 'sles' || id === 'opensuse' || idLike.includes('suse')) {
        installCmd = `sudo zypper install -y ${name}`;
      } else {
        // Fallback to apt-get
        installCmd = `sudo apt-get update && sudo apt-get install -y ${name}`;
      }
      break; }
    default:
      throw new Error(`Auto-install unsupported on platform ${os.platform()}`);
  }
  return new Promise<void>((resolve, reject) => {
    // Spawn the install command and inherit stdio so user sees progress
    const child = spawn(installCmd, { shell: true, stdio: "inherit" });
    child.on("error", (err) => {
      reject(err);
    });
    child.on("exit", async (code) => {
      if (code === 0) {
        try {
          await detectTools();
          resolve();
        } catch (e) {
          reject(e);
        }
      } else {
        reject(new Error(`Installation of '${name}' failed with exit code ${code}`));
      }
    });
  });
}