import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function createSvgEnv() {
  const rafs = new Map();
  let nextRaf = 1;
  const calls = [];
  let nextActive = true;
  let nextChanged = true;

  class CanvasEl {
    constructor(tag, attrs = {}) {
      this.tagName = tag.toUpperCase();
      this.attributes = { ...attrs };
      this.style = {};
      this.events = {};
      this.width = 0;
      this.height = 0;
      this.frames = 0;
    }
    setAttribute(k, v) { this.attributes[k] = String(v); }
    getAttribute(k) { return this.attributes[k] ?? null; }
    addEventListener(k, fn) { (this.events[k] ??= []).push(fn); }
    removeEventListener(k, fn) { this.events[k] = (this.events[k] ?? []).filter(f => f !== fn); }
    dispatch(k, ev = {}) { for (const fn of this.events[k] ?? []) fn({ preventDefault() {}, clientX: 50, clientY: 50, ...ev }); }
    getBoundingClientRect() { return { left: 0, top: 0, width: 100, height: 100 }; }
    getContext(kind) {
      assert.equal(kind, "2d");
      return { putImageData: (img) => { this.frames++; this.lastImage = img; } };
    }
  }

  const cleanups = [];
  const effects = [];
  const B = {
    bindAttr(node, name, getter) { node.setAttribute(name, typeof getter === "function" ? getter() : getter); },
    svgCached: () => false,
    svgOpen: (key, bytes) => { calls.push(["open", key, bytes?.length ?? 0]); return 42; },
    svgFree: (p) => { calls.push(["free", p]); },
    svgConfigure: (...args) => { calls.push(["cfg", ...args]); return (nextChanged ? 1 : 0) | (nextActive ? 2 : 0); },
    svgPointer: (p, kind, x, y) => {
      calls.push(["ptr", p, kind, x, y]);
      nextActive = true;
      nextChanged = true;
      return 3;
    },
    svgRender: (p, now, w, h) => {
      calls.push(["render", p, w, h]);
      const status = (nextChanged ? 1 : 0) | (nextActive ? 2 : 0);
      const pixels = nextChanged ? new Uint8ClampedArray(w * h * 4) : null;
      return { status, pixels };
    },
  };

  class FakeObserver {
    constructor(cb) { this.cb = cb; }
    observe() {}
    disconnect() { calls.push(["disconnect"]); }
  }

  const context = vm.createContext({
    B,
    el: (tag, attrs) => new CanvasEl(tag, attrs),
    bindLength: (node, prop, val) => { node.style[prop] = `${typeof val === "function" ? val() : val}px`; },
    dark: () => false,
    reduceMotion: () => false,
    themeTick: { get() {} },
    resource: (s) => `/res/${s}`,
    effect: (fn) => { effects.push(fn); fn(); },
    onCleanup: (fn) => cleanups.push(fn),
    fetch: async () => ({ ok: true, arrayBuffer: async () => new Uint8Array([66, 83, 86, 71, 0, 1, 0, 0]).buffer }),
    AbortController: class { constructor() { this.signal = {}; } abort() { calls.push(["abort"]); } },
    IntersectionObserver: FakeObserver,
    ResizeObserver: FakeObserver,
    ImageData: class { constructor(data, w, h) { this.data = data; this.width = w; this.height = h; } },
    window: { devicePixelRatio: 2 },
    document: { hidden: false, addEventListener() {}, removeEventListener() {} },
    performance: { now: () => 1000 },
    requestAnimationFrame: (fn) => { const id = nextRaf++; rafs.set(id, fn); return id; },
    cancelAnimationFrame: (id) => { rafs.delete(id); },
    console,
    Uint8Array,
    Boolean,
    Number,
    Math,
  });

  const src = readFileSync(new URL("../templates/svg.js", import.meta.url), "utf8")
    .replace(/^export function /gm, "function ");
  vm.runInContext(`${src}\nglobalThis.Svg = Svg; globalThis.WebPage = WebPage;`, context);

  return {
    context,
    calls,
    rafs,
    cleanups,
    setState(changed, active) { nextChanged = changed; nextActive = active; },
  };
}

test("Web Svg presenter renders onto <canvas>, pauses RAF when idle, wakes on dblclick, and frees WASM player", async () => {
  const env = createSvgEnv();
  const mounted = [];
  const parent = { append: (node) => mounted.push(node) };

  env.context.Svg(parent, {}, {
    source: "__bern_anim_svg/logo.svg.bsvg",
    label: () => "Animated Logo",
    width: 100,
    height: 100,
    fit: "contain",
    playing: () => true,
    speed: () => 1,
    restart: () => 0,
    interactive: true,
  });

  assert.equal(mounted.length, 1);
  const canvas = mounted[0];
  assert.equal(canvas.tagName, "CANVAS");
  assert.equal(canvas.attributes.class, "bern-svg");
  assert.equal(canvas.attributes["aria-label"], "Animated Logo");

  // Wait for async fetch -> B.svgOpen -> initial renderFrame.
  await new Promise((r) => setTimeout(r, 10));
  assert.equal(canvas.width, 200);
  assert.equal(canvas.height, 200);
  assert.equal(canvas.frames, 1);
  assert.equal(env.rafs.size, 1, "active animation schedules one RAF");

  // Simulate entrance finishing (`active = false`): RAF stops at 0% idle CPU.
  env.setState(false, false);
  const [, tickFn] = [...env.rafs.entries()][0];
  env.rafs.clear();
  tickFn(2600);
  assert.equal(env.rafs.size, 0, "idle scene schedules no RAF");

  // Double-click wakes the player and schedules RAF again.
  canvas.dispatch("dblclick");
  assert.ok(env.calls.some((c) => c[0] === "ptr" && c[2] === 1));
  assert.equal(env.rafs.size, 1, "double-click wakes RAF loop");
  assert.equal(canvas.frames, 2);

  // Cleanup frees the WASM player handle and cancels RAF.
  for (const fn of env.cleanups) fn();
  assert.equal(env.rafs.size, 0);
  assert.ok(env.calls.some((c) => c[0] === "free" && c[1] === 42));
});
