import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

async function harness() {
  const source = await readFile(new URL('../templates/features.js', import.meta.url), 'utf8');
  const microtasks = [], frames = [], timeouts = [], ghosts = [];
  class Picture {
    constructor(rect) { this.rect = rect; this.style = { visibility: '', borderRadius: '22px' }; this.isConnected = true; }
    matches(name) { return name === 'img'; }
    querySelector() { return null; }
    querySelectorAll() { return []; }
    removeAttribute() {}
    setAttribute() {}
    cloneNode() { const copy = new Picture(this.rect); copy.style = { ...this.style }; return copy; }
    getBoundingClientRect() {
      if (this.style.position !== 'fixed') return this.rect;
      return { left: parseFloat(this.style.left), top: parseFloat(this.style.top),
        width: parseFloat(this.style.width), height: parseFloat(this.style.height) };
    }
    remove() { this.isConnected = false; const i = ghosts.indexOf(this); if (i >= 0) ghosts.splice(i, 1); }
  }
  const ctx = vm.createContext({
    queueMicrotask: (fn) => microtasks.push(fn), requestAnimationFrame: (fn) => frames.push(fn),
    setTimeout: (fn) => timeouts.push(fn),
    getComputedStyle: (node) => node.style,
    document: { body: { append: (node) => { node.isConnected = true; ghosts.push(node); } } },
  });
  const spring = source.slice(source.indexOf('export class Spring'), source.indexOf('/// The transform of an element')).replace('export ', '');
  const heroes = source.slice(source.indexOf('const heroes = new Map();'), source.indexOf('// MARK: - Element decoration'));
  vm.runInContext(`let reduced = false; const running = new Set(); function frame() {}\n${spring}\n${heroes}\n` +
    'globalThis.api = { heroEnter, heroLeave, running, reduce: () => { reduced = true; } };', ctx);
  const flush = () => { while (microtasks.length) microtasks.shift()(); };
  const step = (dt) => { for (const spring of [...ctx.api.running]) spring.step(dt); };
  return { ...ctx.api, Picture, ghosts, flush, frames, timeouts, step };
}

test('image hero waits for scaled scene layout and flies outside clipped parents', async () => {
  const h = await harness();
  const source = new h.Picture({ left: 12, top: 600, width: 208, height: 117 });
  h.heroEnter('main:frame', source, [0.48, 0.86]); h.flush();
  const destination = new h.Picture({ left: 0, top: 0, width: 0, height: 0 });
  h.heroEnter('main:frame', destination, [0.48, 0.86]); h.flush();
  assert.equal(destination.style.visibility, 'hidden');
  assert.equal(h.ghosts.length, 0);
  destination.rect = { left: 58, top: 36, width: 604, height: 340 };
  h.frames.shift()();
  assert.equal(h.ghosts.length, 2);
  assert.equal(source.style.visibility, 'hidden');
  assert.equal(destination.style.transform, undefined, 'the scaled real node is never translated in viewport units');
  h.step(0.1);
  const rect = h.ghosts[1].getBoundingClientRect();
  assert.ok(rect.width > 208 && rect.width < 604);
  assert.ok(rect.top < 600 && rect.top > 36);
  h.step(2);
  assert.equal(h.ghosts.length, 0);
  assert.equal(destination.style.visibility, '');
  assert.equal(source.style.visibility, '');
});

test('reversing a running image hero continues from its current rectangle', async () => {
  const h = await harness();
  const big = new h.Picture({ left: 103, top: 64, width: 1074, height: 604 });
  h.heroEnter('main:frame', big, [0.48, 0.86]); h.flush();
  const mini = new h.Picture({ left: 12, top: 600, width: 208, height: 117 });
  h.heroEnter('main:frame', mini, [0.48, 0.86]); h.flush(); h.step(0.1);
  const before = h.ghosts[1].getBoundingClientRect();
  h.heroLeave('main:frame', mini); mini.isConnected = false;
  const restored = new h.Picture(big.rect);
  h.heroEnter('main:frame', restored, [0.48, 0.86]); h.flush();
  assert.equal(h.ghosts.length, 2, 'no abandoned overlay from the first flight');
  assert.deepEqual(h.ghosts[1].getBoundingClientRect(), before);
  h.step(2);
  assert.equal(h.ghosts.length, 0);
  assert.equal(restored.style.visibility, '');
});

test('reduced motion and removed destinations clean up running image flights', async () => {
  for (const remove of [false, true]) {
    const h = await harness();
    const source = new h.Picture({ left: 12, top: 600, width: 208, height: 117 });
    const destination = new h.Picture({ left: 103, top: 64, width: 1074, height: 604 });
    h.heroEnter('main:frame', source, [0.48, 0.86]); h.flush();
    h.heroEnter('main:frame', destination, [0.48, 0.86]); h.flush();
    if (remove) destination.isConnected = false; else h.reduce();
    h.step(0.05);
    assert.equal(h.ghosts.length, 0);
    assert.equal(h.running.size, 0);
    assert.equal(source.style.visibility, '');
    assert.equal(destination.style.visibility, '');
  }
});
