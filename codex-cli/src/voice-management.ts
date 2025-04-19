import { IncomingMessage, ServerResponse } from 'http';
import path from 'path';
import fs from 'fs';
import { spawn } from 'child_process';

// Directory where whisper.cpp is cloned and built
const WHISPER_DIR = path.resolve(__dirname, '../whisper.cpp');
const WHISPER_BIN = path.join(WHISPER_DIR, 'main');

/**
 * Handles HTTP requests under /voice/* routes.
 * Returns true if the request was handled.
 */
export async function handleVoiceRequest(req: IncomingMessage, res: ServerResponse): Promise<boolean> {
  const url = req.url || '';
  if (req.method === 'POST' && url.startsWith('/voice/upload')) {
    const tmpPath = path.join(process.cwd(), 'voice_upload');
    const writeStream = fs.createWriteStream(tmpPath);
    req.pipe(writeStream);
    writeStream.on('finish', () => {
      const whisper = spawn(WHISPER_BIN, ['-m', 'models/ggml-base.en.bin', '-f', tmpPath]);
      let output = '';
      whisper.stdout.on('data', (chunk) => { output += chunk.toString(); });
      whisper.on('close', () => {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ transcription: output }));
        fs.unlink(tmpPath, () => {});
      });
    });
    return true;
  }
  if (req.method === 'POST' && url.startsWith('/voice/segment')) {
    let body = '';
    req.on('data', (chunk) => { body += chunk; });
    req.on('end', () => {
      // TODO: Actual speaker segmentation
      const segments = body.split('.').map(s => s.trim()).filter(Boolean)
        .map((text, i) => ({ speaker: `speaker_${i+1}`, text }));
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ segments }));
    });
    return true;
  }
  return false;
}