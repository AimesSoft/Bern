import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";
import vm from "node:vm";

const source = await readFile(new URL("../templates/bern.js", import.meta.url), "utf8");

async function harness(t, { bound = false, startOnly = false, reserved = false, location = "bindgen/0123456789abcdef/bern_bindgen_bg.js" } = {}) {
  const root = await mkdtemp(join(tmpdir(), "bern-wasm-loader-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "nested/bindgen/0123456789abcdef"), { recursive: true });
  await writeFile(join(root, "package.json"), '{"type":"module"}');
  if (bound) {
    await writeFile(join(root, "nested/bindgen/0123456789abcdef/bern_bindgen_bg.js"), `
      let wasm;
      export const version = new URL(import.meta.url).search;
      export function __wbg_set_wasm(value) { wasm = value; }
      export function probe() { return wasm.memory; }
    `);
  }
  const calls = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  const exports = { memory, bern_bytes_alloc: () => { calls.push("allocate"); return 0; } };
  let imports;
  let context;
  if (bound) exports.__wbindgen_start = () => {
    assert.equal(imports["./bern_bindgen_bg.js"].probe(), memory, "bindings initialized before start");
    assert.equal(context.testHost.exports, exports, "Bern imports can access memory during start");
    assert.equal(imports["./bern_bindgen_bg.js"].version, "?v=build-two");
    calls.push("start");
  };
  context = vm.createContext({
    console, TextDecoder, TextEncoder, Uint8Array, DataView, URL, setTimeout, clearTimeout,
    document: { baseURI: pathToFileURL(join(root, "index.html")).href, addEventListener() {} },
    fetch: async (url) => {
      assert.equal(url, "nested/app.wasm?v=build-two");
      return { ok: true, arrayBuffer: async () => new ArrayBuffer(0) };
    },
    WebAssembly: {
      compile: async () => { calls.push("compile"); return {}; },
      Module: {
        imports: () => bound && !startOnly ? [{ module: "./bern_bindgen_bg.js" }] : [],
        exports: () => bound ? [{ name: "__wbindgen_start" }] : [],
        customSections: (_, name) => {
          assert.equal(name, "bern.bindgen");
          return location ? [new TextEncoder().encode(location).buffer] : [];
        },
      },
      instantiate: async (_, provided) => {
        imports = provided;
        assert.equal(typeof imports.bern.storage_read, "function");
        assert.equal(imports.custom.ping(), 42, "extension imports survive");
        calls.push("instantiate");
        return { exports };
      },
    },
  });
  vm.runInContext(source.replace(/^export /gm, "") + `
    globalThis.testHost = host;
    installWebExtension(() => ({ imports: ${reserved
      ? '{ "./bern_bindgen_bg.js": {} }'
      : '{ custom: { ping: () => 42 } }'} }));
  `, context, { importModuleDynamically: vm.constants.USE_MAIN_CONTEXT_DEFAULT_LOADER });
  return { load: () => context.testHost.load("nested/app.wasm?v=build-two", "app"), calls };
}

test("plain modules load without requesting binding scripts", async (t) => {
  const h = await harness(t);
  await h.load();
  assert.deepEqual(h.calls, ["compile", "instantiate", "allocate"]);
});

test("bindings preserve Bern and extension imports and start before application allocation", async (t) => {
  const h = await harness(t, { bound: true });
  await h.load();
  assert.deepEqual(h.calls, ["compile", "instantiate", "start", "allocate"]);
});

test("a bindgen start export alone still initializes its bindings", async (t) => {
  const h = await harness(t, { bound: true, startOnly: true });
  await h.load();
  assert.deepEqual(h.calls, ["compile", "instantiate", "start", "allocate"]);
});

test("missing or unsafe binding locations fail before loading JavaScript", async (t) => {
  for (const location of ["", "https://example.invalid/code.js", "bindgen/../code.js"]) {
    const h = await harness(t, { bound: true, location });
    await assert.rejects(h.load(), /Missing or invalid wasm-bindgen artifact location/);
    assert.deepEqual(h.calls, ["compile"]);
  }
});

test("extensions cannot replace generated binding imports", async (t) => {
  const h = await harness(t, { bound: true, reserved: true });
  await assert.rejects(h.load(), /Reserved WASM import module/);
  assert.deepEqual(h.calls, ["compile"]);
});
