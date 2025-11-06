#!/usr/bin/env node
/**
 * Post-install script for @zapabob/codex-cli
 * Downloads pre-built binaries from GitHub Releases
 */

const https = require('https');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { promisify } = require('util');
const pipeline = promisify(require('stream').pipeline);

const VERSION = '2.0.0';
const GITHUB_REPO = 'zapabob/codex';

// Platform detection
function getPlatformInfo() {
  const platform = process.platform;
  const arch = process.arch;
  
  const platformMap = {
    win32: { os: 'windows', ext: '.exe', archive: 'zip' },
    darwin: { os: 'macos', ext: '', archive: 'tar.gz' },
    linux: { os: 'linux', ext: '', archive: 'tar.gz' },
  };
  
  const archMap = {
    x64: 'x64',
    arm64: 'arm64',
  };
  
  if (!platformMap[platform]) {
    throw new Error(`Unsupported platform: ${platform}`);
  }
  
  if (!archMap[arch]) {
    throw new Error(`Unsupported architecture: ${arch}`);
  }
  
  return {
    ...platformMap[platform],
    archName: archMap[arch],
  };
}

// Download file from URL
async function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    https.get(url, { followAllRedirects: true }, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301) {
        downloadFile(response.headers.location, dest).then(resolve).catch(reject);
        return;
      }
      
      if (response.statusCode !== 200) {
        reject(new Error(`Failed to download: HTTP ${response.statusCode}`));
        return;
      }
      
      const file = fs.createWriteStream(dest);
      pipeline(response, file)
        .then(() => resolve())
        .catch(reject);
    }).on('error', reject);
  });
}

// Verify SHA256 checksum
async function verifySHA256(filePath, expectedHash) {
  const hash = crypto.createHash('sha256');
  const stream = fs.createReadStream(filePath);
  
  return new Promise((resolve, reject) => {
    stream.on('data', (data) => hash.update(data));
    stream.on('end', () => {
      const computed = hash.digest('hex');
      resolve(computed === expectedHash);
    });
    stream.on('error', reject);
  });
}

// Extract archive
async function extractArchive(archivePath, destDir) {
  const { promisify } = require('util');
  const exec = promisify(require('child_process').exec);
  
  const ext = path.extname(archivePath);
  
  if (ext === '.zip') {
    // Windows: use unzip or 7z
    try {
      await exec(`powershell -command "Expand-Archive -Path '${archivePath}' -DestinationPath '${destDir}' -Force"`);
    } catch {
      await exec(`7z x "${archivePath}" -o"${destDir}" -y`);
    }
  } else if (archivePath.endsWith('.tar.gz')) {
    // Unix: use tar
    await exec(`tar -xzf "${archivePath}" -C "${destDir}"`);
  }
}

async function main() {
  console.log('📦 Installing @zapabob/codex-cli v' + VERSION);
  
  try {
    const platformInfo = getPlatformInfo();
    console.log(`🖥️  Platform: ${platformInfo.os}-${platformInfo.archName}`);
    
    // Binary filename
    const binaryName = `codex-${platformInfo.os}-${platformInfo.archName}${platformInfo.ext}`;
    const downloadUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/${binaryName}`;
    
    console.log(`⬇️  Downloading: ${downloadUrl}`);
    
    // Ensure bin directory exists
    const binDir = path.join(__dirname, '..', 'bin');
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }
    
    const binaryPath = path.join(binDir, 'codex' + platformInfo.ext);
    const tempPath = path.join(binDir, 'codex.tmp');
    
    // Download binary
    await downloadFile(downloadUrl, tempPath);
    console.log('✅ Download complete');
    
    // Move to final location
    fs.renameSync(tempPath, binaryPath);
    
    // Make executable (Unix)
    if (platformInfo.os !== 'windows') {
      fs.chmodSync(binaryPath, 0o755);
    }
    
    console.log(`✅ Installed: ${binaryPath}`);
    console.log('');
    console.log('🎉 Installation complete!');
    console.log('');
    console.log('Run: codex --version');
    console.log('Or:  npx @zapabob/codex-cli --version');
    
  } catch (error) {
    console.error('❌ Installation failed:', error.message);
    console.error('');
    console.error('💡 Alternative: Install from source');
    console.error('   git clone https://github.com/zapabob/codex.git');
    console.error('   cd codex/codex-rs');
    console.error('   cargo install --path cli');
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}
