// Integration check against an isolated, running basic-demo server.
// Usage: node scripts/smoke-webui.mjs http://127.0.0.1:3000
import assert from 'node:assert/strict';

const base = new URL(process.argv[2] ?? 'http://127.0.0.1:3000');
const request = (path, options) => fetch(new URL(path, base), options);
const state = async () => (await request('/api/state')).json();
const before = await state();
assert.equal(typeof before.engine_id, 'string');
assert.equal(typeof before.revision, 'string');

async function connect() {
  const url = new URL('/api/ws', base);
  url.protocol = base.protocol === 'https:' ? 'wss:' : 'ws:';
  const socket = new WebSocket(url);
  const messages = [];
  const waiters = new Set();
  socket.addEventListener('message', (event) => {
    messages.push(JSON.parse(event.data));
    for (const check of waiters) check();
  });
  const wait = (predicate) => new Promise((resolve, reject) => {
    const timer = setTimeout(() => { waiters.delete(check); reject(new Error('WebSocket response timed out')); }, 5000);
    function check() {
      const index = messages.findIndex(predicate);
      if (index < 0) return;
      clearTimeout(timer);
      waiters.delete(check);
      resolve(messages.splice(index, 1)[0]);
    }
    waiters.add(check);
    check();
  });
  const welcome = await wait((m) => m.type === 'welcome');
  assert.equal(welcome.protocol_version, 1);
  assert.equal(welcome.engine_id, before.engine_id);
  return { socket, wait };
}

const clients = await Promise.all([connect(), connect()]);
let serial = 0;
function envelope(command, overrides = {}) {
  return { protocol_version: 1, request_id: `smoke-${++serial}`, engine_id: before.engine_id, command, params: {}, ...overrides };
}
async function post(body, headers = {}) {
  const response = await request('/api/command', {
    method: 'POST', headers: { 'Content-Type': 'application/json', ...headers }, body: JSON.stringify(body),
  });
  return { response, body: await response.json().catch(() => null) };
}

try {
  const mutation = envelope('counter.increment');
  clients[0].socket.send(JSON.stringify(mutation));
  const result = await clients[0].wait((m) => m.type === 'result' && m.request_id === mutation.request_id);
  assert.equal(result.ok, true);
  const snapshots = await Promise.all(clients.map((c) => c.wait((m) => m.type === 'snapshot' && m.snapshot.revision === result.revision)));
  assert.deepEqual(snapshots[0].snapshot, snapshots[1].snapshot);
  assert.equal(snapshots[0].snapshot.values['counter.value'], before.values['counter.value'] + 1);

  const after = await state();
  for (const invalid of [
    envelope('counter.increment', { protocol_version: 0 }),
    envelope('counter.increment', { engine_id: 'wrong-instance' }),
    envelope('unknown.command'),
    envelope('counter.increment', { params: { unexpected: 1 } }),
  ]) {
    const rejected = await post(invalid);
    assert.equal(rejected.body?.ok, false);
    assert.deepEqual(await state(), after);
  }
  const badShape = await post({ ...envelope('counter.increment'), hidden: true });
  assert.equal(badShape.response.ok, false);
  assert.deepEqual(await state(), after);
  const crossOrigin = await post(envelope('counter.increment'), { Origin: 'https://untrusted.example' });
  assert.equal(crossOrigin.response.status, 403);
  assert.deepEqual(await state(), after);

  const viaHttp = await post(envelope('counter.increment'));
  assert.equal(viaHttp.body.ok, true);
  assert.equal((await state()).values['counter.value'], before.values['counter.value'] + 2);

  const manifest = await (await request('/build.json')).json();
  assert.equal(manifest.mode, 'ui');
  assert.equal(manifest.engine_included, false);
  const wasm = await request('/pkg/bern_web_runtime_bg.wasm');
  assert.equal(wasm.ok, true);
  assert.match(wasm.headers.get('content-type'), /application\/wasm/);
  console.log('PASS: two WebSocket clients and HTTP share one engine; invalid requests and cross-origin writes are rejected; UI-only WASM assets are served.');
} finally {
  for (const client of clients) client.socket.close();
}
