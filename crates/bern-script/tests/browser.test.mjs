import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {Worker as NodeWorker} from 'node:worker_threads';
import vm from 'node:vm';

test('browser workers preserve contexts and return JSON, promises and errors', async () => {
  const sources = new Map(), workers = new Set(), replies = new Map();
  let batch = [
    {id: '1', context: 'a', source: 'var count = 1; ++count'},
    {id: '2', context: 'a', source: '++count'},
    {id: '3', context: 'b', source: 'typeof count'},
    {id: '4', context: 'a', source: 'Promise.resolve({answer: 42})'},
    {id: '5', context: 'a', source: "throw new Error('broken')"},
  ];
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
    exports: {bern_script_start() {}, bern_script_stop() { stopped = true; },
      bern_script_complete(bytes) { const reply = JSON.parse(new TextDecoder().decode(bytes)); replies.set(reply.id, reply); }},
    withBytes(bytes, fn) { fn(bytes, bytes.length); },
    buffer() { const value = batch.length ? JSON.stringify(batch) : ''; batch = []; return new TextEncoder().encode(value); },
  };
  const context = vm.createContext({host, Worker, performance, setInterval, clearInterval, TextDecoder, TextEncoder,
    addEventListener() {}, Blob: class {constructor(parts) {this.source = parts.join('');}},
    URL: {createObjectURL(blob) { sources.set('worker', blob.source); return 'worker'; }, revokeObjectURL(url) { sources.delete(url); }}});
  vm.runInContext(readFileSync(new URL('../../bern-target-api/templates/script/host.js', import.meta.url), 'utf8') + '\nglobalThis.stop = startScriptHost(host);', context);
  try {
    const deadline = Date.now() + 10000;
    while (replies.size < 5 && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10));
    assert.equal(replies.get('1')?.value, 2); assert.equal(replies.get('2')?.value, 3);
    assert.equal(replies.get('3')?.value, 'undefined'); assert.deepEqual(replies.get('4')?.value, {answer: 42});
    assert.equal(replies.get('5')?.error, 'Error: broken');
    batch = [{id: '6', context: 'a', source: null}];
    await new Promise(resolve => setTimeout(resolve, 40));
    assert.equal(workers.size, 1);
  } finally { context.stop(); }
  assert.equal(workers.size, 0); assert.equal(stopped, true);
});
