import { IncomingMessage, ServerResponse } from 'http';
import fs from 'fs';
import path from 'path';
// Directory where knowledge bases are stored
const KBASES_DIR = path.join(process.cwd(), 'knowledge_bases');
if (!fs.existsSync(KBASES_DIR)) fs.mkdirSync(KBASES_DIR, { recursive: true });

// In-memory vector store placeholder
let vectorStore: any = null;

/** Handles HTTP requests under /knowledge/* routes. */
export async function handleKnowledgeRequest(req: IncomingMessage, res: ServerResponse): Promise<boolean> {
  const url = req.url || '';
  // List all knowledge bases on disk
  if (req.method === 'GET' && url.startsWith('/knowledge/list')) {
    try {
      const entries = fs.readdirSync(KBASES_DIR, { withFileTypes: true });
      const bases = entries.filter(e => e.isDirectory()).map(e => e.name);
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ bases }));
    } catch (err) {
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Failed to list knowledge bases' }));
    }
    return true;
  }
  // Create store: POST /knowledge/create
  if (req.method === 'POST' && url.startsWith('/knowledge/create')) {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', () => {
      // TODO: parse docs, chunk, embed, and build FAISS store
      vectorStore = {};
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ success: true }));
    });
    return true;
  }
  // Add documents: POST /knowledge/add
  if (req.method === 'POST' && url.startsWith('/knowledge/add')) {
    // TODO: add new docs to store
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ success: true }));
    return true;
  }
  // Delete store: DELETE /knowledge/store
  if (req.method === 'DELETE' && url.startsWith('/knowledge/store')) {
    vectorStore = null;
    res.writeHead(200);
    res.end('deleted');
    return true;
  }
  // Download store: GET /knowledge/download
  if (req.method === 'GET' && url.startsWith('/knowledge/download')) {
    // TODO: stream store file
    res.writeHead(404);
    res.end();
    return true;
  }
  // Search store: GET /knowledge/search?query=...
  if (req.method === 'GET' && url.startsWith('/knowledge/search')) {
    const q = new URL(req.url!, `http://${req.headers.host}`).searchParams.get('query') || '';
    // TODO: perform search on vectorStore
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ results: [] }));
    return true;
  }
  return false;
}