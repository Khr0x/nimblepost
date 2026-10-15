import { createServer } from 'node:http';
import { hash } from './performance-report.mjs';

const MiB = 2 ** 20;
export function responseFixtures() {
  const fill = (pattern, bytes) => {
    const length = Buffer.byteLength(pattern);
    return Buffer.from(pattern.repeat(Math.floor(bytes / length)) + 'x'.repeat(bytes % length));
  };
  const row = '{"value":"NP013_JSON_雪🙂_' + 'x'.repeat(100) + '"},\n';
  const prefix = '[\n', suffix = '{}\n]';
  const count = Math.floor((10 * MiB - Buffer.byteLength(prefix + suffix)) / Buffer.byteLength(row));
  const json = Buffer.from(prefix + row.repeat(count) + ' '.repeat(10 * MiB - Buffer.byteLength(prefix + suffix) - count * Buffer.byteLength(row)) + suffix);
  return { json, line: fill('NP013_LINE_雪🙂|', 2 * MiB), text: fill('NP013_TEXT_雪🙂\n', 2 * MiB) };
}
export const fixtureIdentity = payloads => Object.fromEntries(Object.entries(payloads).map(([name, value]) => [name, { bytes: value.length, sha256: hash(value) }]));

export async function serveResponses() {
  const payloads = responseFixtures(), traffic = [];
  const server = createServer(async (request, response) => {
    const name = request.url.slice(1);
    const record = { name, bytesWritten: 0, finished: false, closed: false, secretReceived: request.headers['x-probe-secret'] === 'NP013_RUNTIME_SECRET_CANARY' }; traffic.push(record);
    response.on('finish', () => { record.finished = true; });
    response.on('close', () => { record.closed = true; });
    response.on('error', () => {});
    try {
      const payload = payloads[name];
      if (payload) {
        response.writeHead(200, { 'Content-Type': name === 'json' ? 'application/json; charset=utf-8' : 'text/plain; charset=utf-8', 'Content-Length': payload.length });
        response.end(payload); record.bytesWritten = payload.length;
      } else if (name === 'excess' || name === 'slow') {
        // Deliberately omit Content-Length: the engine must enforce the real stream limit.
        response.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' });
        const block = Buffer.alloc(64 * 1024, 120), limit = (name === 'excess' ? 17 : 10) * MiB;
        while (record.bytesWritten < limit && !response.destroyed) {
          if (!response.write(block)) await new Promise(resolve => {
            const done = () => { response.off('drain', done); response.off('close', done); resolve(); };
            response.once('drain', done); response.once('close', done);
          });
          record.bytesWritten += block.length;
          if (name === 'slow') await new Promise(resolve => setTimeout(resolve, 100));
        }
        if (!response.destroyed) response.end();
      } else { response.writeHead(404); response.end(); }
    } catch { response.destroy(); }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  return { url: `http://127.0.0.1:${server.address().port}`, traffic, fixtures: fixtureIdentity(payloads), close: () => new Promise(resolve => { server.close(resolve); server.closeAllConnections(); }) };
}
