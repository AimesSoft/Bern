import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {Worker as NodeWorker} from 'node:worker_threads';
import vm from 'node:vm';

function createHarness(onCli) {
  const sources = new Map(), workers = new Set(), replies = new Map();
  let batch = [];
  let stopped = false;
  class Worker {
    constructor(url) {
      this.worker = new NodeWorker(`const {parentPort} = require('node:worker_threads');
        globalThis.onmessage = undefined;
        globalThis.postMessage = value => parentPort.postMessage(value);
        ${sources.get(url)}
        parentPort.on('message', data => onmessage({data}));`, {eval: true});
      workers.add(this);
      this.worker.on('message', data => this.onmessage?.({data}));
      this.worker.on('error', error => this.onerror?.(error));
    }
    postMessage(value) { this.worker.postMessage(value); }
    terminate() { workers.delete(this); return this.worker.terminate(); }
  }
  const host = {
    exports: {
      bern_script_start() {},
      bern_script_stop() { stopped = true; },
      bern_script_complete(bytes) {
        const reply = JSON.parse(new TextDecoder().decode(bytes));
        replies.set(reply.id, reply);
      },
      bern_script_cli(bytes) {
        const request = JSON.parse(new TextDecoder().decode(bytes));
        if (onCli) onCli(request, item => batch.push(item));
      },
    },
    withBytes(bytes, fn) { fn(bytes, bytes.length); },
    buffer() {
      const value = batch.length ? JSON.stringify(batch) : '';
      batch = [];
      return new TextEncoder().encode(value);
    },
  };
  const context = vm.createContext({
    host, Worker, performance, setInterval, clearInterval, TextDecoder, TextEncoder,
    addEventListener() {},
    Blob: class { constructor(parts) { this.source = parts.join(''); } },
    URL: {
      createObjectURL(blob) { sources.set('worker', blob.source); return 'worker'; },
      revokeObjectURL(url) { sources.delete(url); },
    },
  });
  vm.runInContext(
    readFileSync(new URL('../../bern-target-api/templates/script/host.js', import.meta.url), 'utf8') +
      '\nglobalThis.stop = startScriptHost(host);',
    context,
  );
  return {
    context,
    workers,
    replies,
    push(items) { batch.push(...items); },
    isStopped() { return stopped; },
  };
}

test('browser workers preserve contexts and return JSON, promises and errors', async () => {
  const harness = createHarness();
  harness.push([
    {id: '1', context: 'a', source: 'var count = 1; ++count'},
    {id: '2', context: 'a', source: '++count'},
    {id: '3', context: 'b', source: 'typeof count'},
    {id: '4', context: 'a', source: 'Promise.resolve({answer: 42})'},
    {id: '5', context: 'a', source: "throw new Error('broken')"},
  ]);
  try {
    const deadline = Date.now() + 10000;
    while (harness.replies.size < 5 && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10));
    assert.equal(harness.replies.get('1')?.value, 2);
    assert.equal(harness.replies.get('2')?.value, 3);
    assert.equal(harness.replies.get('3')?.value, 'undefined');
    assert.deepEqual(harness.replies.get('4')?.value, {answer: 42});
    assert.equal(harness.replies.get('5')?.error, 'Error: broken');
    harness.push([{id: '6', context: 'a', source: null}]);
    await new Promise(resolve => setTimeout(resolve, 40));
    assert.equal(harness.workers.size, 1);
  } finally {
    harness.context.stop();
  }
  assert.equal(harness.workers.size, 0);
  assert.equal(harness.isStopped(), true);
});

test('browser workers route bern.cli calls and errors through the host bridge', async () => {
  const seen = [];
  const harness = createHarness((request, enqueue) => {
    seen.push(request);
    if (request.input.kind === 'command' && request.input.target === 'denied.op') {
      enqueue({
        cli_reply: {
          call_id: request.call_id,
          context: request.context,
          error: {code: 'permission_denied', message: 'not permitted'},
        },
      });
      return;
    }
    enqueue({
      cli_reply: {
        call_id: request.call_id,
        context: request.context,
        value: {echo: request.input},
      },
    });
  });
  harness.push([
    {
      id: 'cli-1',
      context: 'c1',
      source: `(async () => {
        const line = await bern.cli("media pause --player-id main");
        const args = await bern.cli(["state"]);
        const cmd = await bern.cli("media.seek", { position_ms: 9000 }, { session: "s1" });
        let errCode = null;
        try { await bern.cli("denied.op", {}); } catch (e) { errCode = e instanceof Error ? e.code : null; }
        return { line: line.echo.kind, args: args.echo.args, session: cmd.echo.session, errCode };
      })()`,
    },
  ]);
  try {
    const deadline = Date.now() + 10000;
    while (!harness.replies.has('cli-1') && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10));
    assert.deepEqual(harness.replies.get('cli-1')?.value, {
      line: 'line',
      args: ['state'],
      session: 's1',
      errCode: 'permission_denied',
    });
    assert.equal(seen.length, 4);
  } finally {
    harness.context.stop();
  }
});
