// Run with: node --test crates/bern-backend-web/tests/runtime.mjs
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { Timer, Scope, within, cell, accordionWidths, layoutIntent, fixedSize } from "../templates/bern.js";

function layoutNode(classes, children = [], axis = null, tagName = "DIV") {
  const names = new Set(classes.split(" "));
  return { nodeType: 1, tagName, classList: {
    contains: (name) => names.has(name),
    toggle: (name, value) => { if (value) names.add(name); else names.delete(name); },
  }, children, bernLayoutAxis: axis };
}

test("query wrappers gain and release scroll layout intent as asynchronous content changes", async () => {
  const source = await readFile(new URL("../templates/bern.js", import.meta.url), "utf8");
  const observers = [];
  const context = vm.createContext({ console, MutationObserver: class {
    constructor(callback) { this.callback = callback; observers.push(this); }
    observe() {}
    disconnect() { this.disconnected = true; }
  } });
  vm.runInContext(source.replace(/^export /gm, "") + "\nglobalThis.layoutTest = { Scope, within, trackLayout };", context);
  const { Scope: LayoutScope, within: layoutWithin, trackLayout } = context.layoutTest;
  const content = layoutNode("bern-query-content");
  const query = layoutNode("bern-query", [content]);
  const owner = new LayoutScope(null);
  layoutWithin(owner, () => trackLayout(query, "column"));
  assert.equal(query.classList.contains("bern-layout-height"), false);
  const scroll = layoutNode("bern-scroll bern-fill-width bern-fill-height");
  const page = layoutNode("bern-stack bern-vstack", [layoutNode("bern-gesture", [scroll])], "column");
  content.children.push(page);
  observers[0].callback([{ addedNodes: [page], removedNodes: [] }]);
  assert.equal(query.classList.contains("bern-layout-height"), true);
  assert.equal(content.classList.contains("bern-layout-height"), true);
  assert.equal(content.classList.contains("bern-layout-width"), true);
  content.children = [];
  observers[0].callback([{ addedNodes: [], removedNodes: [page] }]);
  assert.equal(query.classList.contains("bern-layout-height"), false);
  assert.equal(content.classList.contains("bern-layout-height"), false);
  owner.dispose();
  assert.equal(observers[0].disconnected, true);
});

test("spacers expand only their stack axis while fixed drawing frames stop propagation", () => {
  const spacer = layoutNode("bern-spacer");
  const row = layoutNode("bern-stack bern-hstack", [spacer], "row");
  const column = layoutNode("bern-stack bern-vstack", [spacer], "column");
  assert.equal(layoutIntent(row, "width"), true);
  assert.equal(layoutIntent(row, "height"), false);
  assert.equal(layoutIntent(column, "width"), false);
  assert.equal(layoutIntent(column, "height"), true);
  const canvas = layoutNode("bern-canvas bern-fill-width bern-fill-height");
  const dot = layoutNode("bern-layers bern-sized-width bern-sized-height", [canvas], "layers");
  const dots = layoutNode("bern-stack bern-hstack bern-sized-height", [dot, dot, dot], "row");
  assert.equal(layoutIntent(dots, "width"), false, "fixed dots keep their row intrinsic and centered");
  assert.equal(layoutIntent(dots, "height"), false);
  const footer = layoutNode("bern-layers bern-sized-height", [row, dots], "layers");
  assert.equal(layoutIntent(footer, "width"), true, "the button row's spacer expands the footer");
  assert.equal(layoutIntent(footer, "height"), false);
  const fixed = layoutNode("bern-layers bern-sized-width", [row], "layers");
  assert.equal(layoutIntent(fixed, "width"), false);
  assert.equal(layoutIntent(layoutNode("", [row], null, "DIALOG"), "width"), false, "top-layer dialogs never size their source layout");
});

test("image and text layers determine their size while rails only cover them", () => {
  const image = layoutNode("bern-image bern-sized-width bern-sized-height", [], null, "IMG");
  const rail = layoutNode("bern-rail bern-fill-width", [layoutNode("bern-canvas bern-fill-width bern-sized-height")]);
  const imageWithProgress = layoutNode("bern-layers", [image, rail], "layers");
  const playIcon = layoutNode("bern-label");
  const stage = layoutNode("bern-content-button", [layoutNode("bern-layers", [imageWithProgress, playIcon], "layers")]);
  assert.equal(layoutIntent(stage, "width"), false, "a rail must not expand the fixed stage button and move the adjacent info column");
  assert.equal(layoutIntent(stage, "height"), false);
  assert.equal(layoutIntent(rail, "width"), true, "the rail itself still covers its established parent width");
  const background = layoutNode("bern-rail bern-fill-width", [layoutNode("bern-canvas bern-fill-width bern-fill-height")]);
  const tag = layoutNode("bern-layers", [background, layoutNode("bern-stack", [layoutNode("bern-text")], "row")], "layers");
  assert.equal(layoutIntent(tag, "width"), false);
  assert.equal(layoutIntent(tag, "height"), false);
  const railsOnly = layoutNode("bern-layers", [background], "layers");
  assert.equal(layoutIntent(railsOnly, "width"), true, "a layer made only of rails still takes the available size");
  assert.equal(layoutIntent(railsOnly, "height"), true);
});

test("buttons that fill make their row fill the column", () => {
  const button = layoutNode("bern-button bern-button-fill", [], null, "BUTTON");
  const row = layoutNode("bern-stack bern-hstack", [button, button], "row");
  assert.equal(layoutIntent(button, "width"), true);
  assert.equal(layoutIntent(button, "height"), false);
  assert.equal(layoutIntent(row, "width"), true, "two buttons that fill share the whole row");
  assert.equal(layoutIntent(layoutNode("bern-stack bern-hstack", [layoutNode("bern-button", [], null, "BUTTON")], "row"), "width"), false);
});

test("a stack of a fixed picture and rails keeps its size beside text that gives way", () => {
  const image = layoutNode("bern-image bern-sized-width bern-sized-height", [], null, "IMG");
  const rail = layoutNode("bern-rail bern-fill-width", [layoutNode("bern-canvas bern-fill-width bern-sized-height")]);
  const thumbnail = layoutNode("bern-layers", [image, rail, rail], "layers");
  assert.equal(fixedSize(thumbnail, "width"), true, "a thumbnail does not shrink under a long title");
  assert.equal(fixedSize(thumbnail, "height"), true);
  const words = layoutNode("bern-layers", [image, layoutNode("bern-text")], "layers");
  assert.equal(fixedSize(words, "width"), false, "a layer with text keeps giving way");
  assert.equal(fixedSize(layoutNode("bern-layers", [rail], "layers"), "width"), false, "rails alone fix nothing");
  assert.equal(fixedSize(layoutNode("bern-layers", [thumbnail], "layers"), "width"), true);
});

test("timer follows enabled state and cancels when its owning view disappears", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const enabled = cell(false);
  const owner = new Scope(null);
  let ticks = 0;
  within(owner, () => Timer(null, {}, { interval: 0.5, enabled: () => enabled.get() }, () => ticks++));
  t.mock.timers.tick(1000);
  assert.equal(ticks, 0);
  enabled.set(true);
  t.mock.timers.tick(499);
  assert.equal(ticks, 0);
  t.mock.timers.tick(1);
  assert.equal(ticks, 1);
  enabled.set(false);
  t.mock.timers.tick(1000);
  assert.equal(ticks, 1);
  enabled.set(true);
  t.mock.timers.tick(500);
  assert.equal(ticks, 2);
  owner.dispose();
  t.mock.timers.tick(1000);
  assert.equal(ticks, 2);
});

test("timer can remove itself during its command without leaving another tick", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const owner = new Scope(null);
  let ticks = 0;
  within(owner, () => Timer(null, {}, { interval: 0.001 }, () => { ticks++; owner.dispose(); }));
  t.mock.timers.tick(49);
  assert.equal(ticks, 0);
  t.mock.timers.tick(1);
  assert.equal(ticks, 1);
  t.mock.timers.tick(1000);
  assert.equal(ticks, 1);
});

test("accordion keeps fixed collapsed columns and fits narrow and overshooting layouts", () => {
  assert.deepEqual(accordionWidths(1000, 86, 8, [0, 0, 1, 0, 0, 0, 0]), [86, 86, 436, 86, 86, 86, 86]);
  for (const total of [0, 20, 500, 1000]) {
    for (const weights of [[0, 1, 0], [-0.8, 1.9, -0.1], [0.2, 0.5, 0.3]]) {
      const widths = accordionWidths(total, 86, 8, weights);
      assert.ok(widths.every((width) => Number.isFinite(width) && width >= 0));
      assert.ok(Math.abs(widths.reduce((a, b) => a + b, 0) - Math.max(0, total - 16)) < 0.0001);
    }
  }
});

async function storageHarness(storage, app = "media", key = "settings") {
  // Exercise the actual WASM imports without starting a browser or engine.
  const source = await readFile(new URL("../templates/bern.js", import.meta.url), "utf8");
  const memory = new WebAssembly.Memory({ initial: 1 });
  let imports;
  const context = vm.createContext({
    console, TextDecoder, TextEncoder, Uint8Array, DataView, setTimeout, clearTimeout,
    localStorage: storage,
    fetch: async () => ({ ok: true, arrayBuffer: async () => new ArrayBuffer(0) }),
    WebAssembly: { instantiate: async (_, provided) => {
      imports = provided.bern;
      return { instance: { exports: { memory, bern_bytes_alloc: () => 0 } } };
    } },
  });
  vm.runInContext(source.replace(/^export /gm, "") + "\nglobalThis.testHost = host;", context);
  await context.testHost.load("test.wasm");
  const bytes = new Uint8Array(memory.buffer);
  const appBytes = new TextEncoder().encode(app);
  const keyBytes = new TextEncoder().encode(key);
  bytes.set(appBytes, 32);
  bytes.set(keyBytes, 64);
  return { imports, bytes, memory, names: [32, appBytes.length, 64, keyBytes.length] };
}

test("WASM storage survives a fresh host and probes length before reading", async () => {
  const values = new Map();
  const storage = { getItem: (key) => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
  const first = await storageHarness(storage);
  assert.equal(first.imports.storage_read(32, 5, 64, 8, 128, 0), -1);
  first.bytes.set([0, 128, 255, 42], 128);
  assert.equal(first.imports.storage_write(32, 5, 64, 8, 128, 4), 0);
  assert.deepEqual(JSON.parse(values.get("bern:media:settings")), [0, 128, 255, 42]);
  const second = await storageHarness(storage);
  assert.equal(second.imports.storage_read(32, 5, 64, 8, 128, 0), 4);
  assert.deepEqual([...second.bytes.slice(128, 132)], [0, 0, 0, 0]);
  assert.equal(second.imports.storage_read(32, 5, 64, 8, 128, 4), 4);
  assert.deepEqual([...second.bytes.slice(128, 132)], [0, 128, 255, 42]);
});

test("WASM storage reports corrupt data and blocked/quota failures", async () => {
  const corrupt = await storageHarness({ getItem: () => "[300]" });
  assert.equal(corrupt.imports.storage_read(32, 5, 64, 8, 128, 4), -2);
  const denied = await storageHarness({ getItem() { throw Error("blocked"); }, setItem() { throw Error("quota"); } });
  assert.equal(denied.imports.storage_read(32, 5, 64, 8, 128, 4), -2);
  assert.equal(denied.imports.storage_write(32, 5, 64, 8, 128, 0), -1);
});

test("WASM storage isolates application namespaces and value keys", async () => {
  const values = new Map([["other-page", "untouched"]]);
  const storage = { getItem: (key) => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
  const media = await storageHarness(storage, "com.example.media", "welcome-v1");
  const anotherApp = await storageHarness(storage, "com.example.player", "welcome-v1");
  const anotherKey = await storageHarness(storage, "com.example.media", "cache-v1");
  media.bytes.set([1, 2], 128);
  assert.equal(media.imports.storage_write(...media.names, 128, 2), 0);
  assert.equal(anotherApp.imports.storage_read(...anotherApp.names, 128, 2), -1);
  assert.equal(anotherKey.imports.storage_read(...anotherKey.names, 128, 2), -1);
  anotherApp.bytes.set([3], 128);
  assert.equal(anotherApp.imports.storage_write(...anotherApp.names, 128, 1), 0);
  const fresh = await storageHarness(storage, "com.example.media", "welcome-v1");
  assert.equal(fresh.imports.storage_read(...fresh.names, 128, 2), 2);
  assert.deepEqual([...fresh.bytes.slice(128, 130)], [1, 2]);
  assert.equal(values.get("bern:com.example.player:welcome-v1"), "[3]");
  assert.equal(values.get("other-page"), "untouched");
});

test("WASM storage preserves empty bytes separately from missing values", async () => {
  const values = new Map();
  const storage = { getItem: (key) => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
  const first = await storageHarness(storage);
  assert.equal(first.imports.storage_read(...first.names, 0, 0), -1);
  assert.equal(first.imports.storage_write(...first.names, 0, 0), 0);
  assert.equal(values.get("bern:media:settings"), "[]");
  const fresh = await storageHarness(storage);
  assert.equal(fresh.imports.storage_read(...fresh.names, 0, 0), 0);
});

test("WASM storage rejects invalid JSON and every non-byte array entry", async () => {
  let value;
  const harness = await storageHarness({ getItem: () => value });
  for (value of ["not-json", "null", "{}", '"text"', "true", "[-1]", "[256]", "[1.5]", "[null]", '["1"]']) {
    assert.equal(harness.imports.storage_read(...harness.names, 128, 4), -2, value);
  }
});

test("WASM storage never partially copies into an undersized destination", async () => {
  const harness = await storageHarness({ getItem: () => "[5,6,7,8]" });
  harness.bytes.fill(99, 128, 132);
  assert.equal(harness.imports.storage_read(...harness.names, 128, 2), 4);
  assert.deepEqual([...harness.bytes.slice(128, 132)], [99, 99, 99, 99]);
  assert.equal(harness.imports.storage_read(...harness.names, 128, 4), 4);
  assert.deepEqual([...harness.bytes.slice(128, 132)], [5, 6, 7, 8]);
});

test("WASM storage reads and writes against the current memory after it grows", async () => {
  const values = new Map();
  const storage = { getItem: (key) => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
  const harness = await storageHarness(storage);
  harness.memory.grow(1);
  const current = new Uint8Array(harness.memory.buffer);
  current.set([10, 20, 30], 65536);
  assert.equal(harness.imports.storage_write(...harness.names, 65536, 3), 0);
  current.fill(0, 65536, 65539);
  assert.equal(harness.imports.storage_read(...harness.names, 65536, 3), 3);
  assert.deepEqual([...current.slice(65536, 65539)], [10, 20, 30]);
});
