// Test under the Pages project prefix to catch accidental root-relative URLs.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname, sep } from 'node:path';
const root = resolve('../_site');
const types = { '.html': 'text/html', '.mjs': 'text/javascript', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
createServer(async (req, res) => {
  try {
    const pathname = new URL(req.url, 'http://localhost').pathname;
    if (!pathname.startsWith('/zpl/')) throw new Error('Not found');
    const path = resolve(root, pathname.slice(5) || 'index.html');
    if (!path.startsWith(root + sep)) throw new Error('Not found');
    const data = await readFile(path);
    res.writeHead(200, { 'Content-Type': types[extname(path)] || 'text/plain' });
    res.end(data);
  } catch { res.writeHead(404); res.end('Not found'); }
}).listen(4173, '127.0.0.1');
