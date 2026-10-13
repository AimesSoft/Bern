// Bern Web runtime. Generated build input; do not edit.
//
// Connects the generated page to the Bern runtime compiled to WebAssembly:
// it decodes typed values, mirrors engine state and window-scoped session
// drafts into fine-grained reactive cells, renders the standard components
// of the Bern Web visual spec, presents windows as virtual windows inside
// the page, and runs every control through the same closures whether the
// user or an external entry operates it. It contains no business logic and
// no literal colors: every color is a `--bern-*` theme variable.

// MARK: - Binary values

export class Writer {
  constructor() {
    this.bytes = [];
  }
  u8(value) {
    this.bytes.push(value & 0xff);
  }
  u32(value) {
    for (let i = 0; i < 4; i++) this.bytes.push((value >>> (8 * i)) & 0xff);
  }
  u64(value) {
    let big = BigInt.asUintN(64, BigInt(value));
    for (let i = 0; i < 8; i++) {
      this.bytes.push(Number(big & 0xffn));
      big >>= 8n;
    }
  }
  i64(value) {
    this.u64(BigInt.asUintN(64, BigInt(value)));
  }
  f64(value) {
    const view = new DataView(new ArrayBuffer(8));
    view.setFloat64(0, value, true);
    for (let i = 0; i < 8; i++) this.bytes.push(view.getUint8(i));
  }
  text(value) {
    const utf8 = new TextEncoder().encode(value);
    this.u32(utf8.length);
    for (const byte of utf8) this.bytes.push(byte);
  }
  finish() {
    return Uint8Array.from(this.bytes);
  }
}

export class DecodeError extends Error {}

export class Reader {
  constructor(bytes) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    this.position = 0;
  }
  take(count) {
    if (this.position + count > this.bytes.length) throw new DecodeError("unexpected end of data");
    const at = this.position;
    this.position += count;
    return at;
  }
  u8() {
    return this.view.getUint8(this.take(1));
  }
  u32() {
    return this.view.getUint32(this.take(4), true);
  }
  u64() {
    return this.view.getBigUint64(this.take(8), true);
  }
  i64() {
    return this.view.getBigInt64(this.take(8), true);
  }
  f64() {
    return this.view.getFloat64(this.take(8), true);
  }
  bool() {
    const value = this.u8();
    if (value > 1) throw new DecodeError("invalid bool");
    return value === 1;
  }
  text() {
    const count = this.u32();
    const at = this.take(count);
    return new TextDecoder("utf-8", { fatal: true }).decode(this.bytes.subarray(at, at + count));
  }
  get done() {
    return this.position === this.bytes.length;
  }
}

/// Decoders and encoders of the built-in scalar types.
export const codec = {
  bool: [(r) => r.bool(), (w, v) => w.u8(v ? 1 : 0)],
  int: [(r) => r.i64(), (w, v) => w.i64(v)],
  float: [(r) => r.f64(), (w, v) => w.f64(v)],
  text: [(r) => r.text(), (w, v) => w.text(v)],
  unit: [() => null, () => {}],
};

export function readOptional(reader, decode) {
  const flag = reader.u8();
  if (flag > 1) throw new DecodeError("invalid optional");
  return flag ? decode(reader) : null;
}

export function writeOptional(writer, value, encode) {
  if (value === null || value === undefined) writer.u8(0);
  else {
    writer.u8(1);
    encode(writer, value);
  }
}

export function readList(reader, decode) {
  const count = reader.u32();
  const items = [];
  for (let i = 0; i < count; i++) items.push(decode(reader));
  return items;
}

export function writeList(writer, value, encode) {
  writer.u32(value.length);
  for (const item of value) encode(writer, item);
}

export function optional([decode, encode]) {
  return [
    (r) => {
      const flag = r.u8();
      if (flag > 1) throw new DecodeError("invalid optional");
      return flag ? decode(r) : null;
    },
    (w, v) => {
      if (v === null || v === undefined) w.u8(0);
      else {
        w.u8(1);
        encode(w, v);
      }
    },
  ];
}

export function list([decode, encode]) {
  return [
    (r) => {
      const count = r.u32();
      const items = [];
      for (let i = 0; i < count; i++) items.push(decode(r));
      return items;
    },
    (w, v) => {
      w.u32(v.length);
      for (const item of v) encode(w, item);
    },
  ];
}

// MARK: - Reactive cells

let observer = null;
let scope = null;
let depth = 0;
const queued = new Set();

/// A value whose readers re-run when it changes.
export class Cell {
  constructor(value) {
    this.value = value;
    this.readers = new Set();
  }
  get() {
    if (observer) {
      this.readers.add(observer);
      observer.sources.add(this);
    }
    return this.value;
  }
  peek() {
    return this.value;
  }
  set(value) {
    if (Object.is(value, this.value)) return;
    this.value = value;
    for (const reader of [...this.readers]) reader.schedule();
  }
}

export function cell(value) {
  return new Cell(value);
}

/// Owns effects, cleanups and child scopes; disposing it ends them all.
export class Scope {
  constructor(parent = scope) {
    this.cleanups = [];
    this.disposed = false;
    if (parent) parent.cleanups.push(() => this.dispose());
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    const cleanups = this.cleanups.splice(0).reverse();
    for (const cleanup of cleanups) cleanup();
  }
}

export function within(owner, body) {
  const saved = scope;
  scope = owner;
  try {
    return body();
  } finally {
    scope = saved;
  }
}

export function onCleanup(cleanup) {
  if (scope) scope.cleanups.push(cleanup);
}

class Effect {
  constructor(body) {
    this.body = body;
    this.sources = new Set();
    this.owner = scope;
    this.inner = null;
    this.disposed = false;
    onCleanup(() => this.dispose());
    this.run();
  }
  schedule() {
    if (this.disposed) return;
    if (depth > 0) queued.add(this);
    else this.run();
  }
  run() {
    if (this.disposed) return;
    for (const source of this.sources) source.readers.delete(this);
    this.sources.clear();
    // Scopes created by the previous run end before the next one starts.
    if (this.inner) this.inner.dispose();
    this.inner = new Scope(null);
    const saved = [observer, scope];
    observer = this;
    scope = this.inner;
    try {
      this.body();
    } catch (error) {
      reportError(error);
    } finally {
      [observer, scope] = saved;
    }
  }
  dispose() {
    this.disposed = true;
    for (const source of this.sources) source.readers.delete(this);
    this.sources.clear();
    if (this.inner) this.inner.dispose();
  }
}

/// Run `body` now and again whenever a cell it read changes.
export function effect(body) {
  return new Effect(body);
}

/// Apply several changes and re-run each affected effect once afterwards.
export function batch(body) {
  depth++;
  try {
    return body();
  } finally {
    depth--;
    if (depth === 0) {
      while (queued.size) {
        const next = [...queued];
        queued.clear();
        for (const effect of next) effect.run();
      }
    }
  }
}

export function untrack(body) {
  const saved = observer;
  observer = null;
  try {
    return body();
  } finally {
    observer = saved;
  }
}

// MARK: - Values

/// Structural equality of decoded values: scalars, BigInt, enums, records
/// and lists.
export function eq(a, b) {
  if (a === b) return true;
  if (typeof a === "bigint" || typeof b === "bigint") return a === b;
  if (a === null || b === null || typeof a !== "object" || typeof b !== "object") return false;
  if (Array.isArray(a)) {
    return Array.isArray(b) && a.length === b.length && a.every((item, i) => eq(item, b[i]));
  }
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((key) => eq(a[key], b[key]));
}

// MARK: - Text

// The session's language and its strings are cells: switching language
// re-runs every binding that shows text, in one update.
const languageCell = cell("en");
const stringsCell = cell({});
let supportedLanguages = [];
let allStrings = {};

function chooseLanguage(supported, fallback) {
  const wanted = [...(navigator.languages || []), navigator.language].filter(Boolean);
  for (const tag of wanted) {
    const exact = supported.find((candidate) => candidate.toLowerCase() === tag.toLowerCase());
    if (exact) return exact;
  }
  for (const tag of wanted) {
    const lower = tag.toLowerCase();
    // zh-CN, zh-SG → zh-Hans; zh-TW, zh-HK → zh-Hant.
    const script = /^zh-(tw|hk|mo)|^zh-hant/.test(lower)
      ? "zh-hant"
      : lower.startsWith("zh")
        ? "zh-hans"
        : lower.split("-")[0];
    const match = supported.find((candidate) => candidate.toLowerCase() === script)
      || supported.find((candidate) => candidate.toLowerCase().split("-")[0] === script);
    if (match) return match;
  }
  return fallback;
}

export function tr(key) {
  return stringsCell.get()[key] ?? key;
}

/// Switch the interface language. Drafts, navigation and windows stay;
/// only text bindings update.
function setLanguage(tag) {
  if (!supportedLanguages.includes(tag)) {
    throw new CommandFailure("unsupported_language", `\`${tag}\` is not supported; supported: ${supportedLanguages.join(", ")}`);
  }
  batch(() => {
    languageCell.set(tag);
    stringsCell.set(allStrings[tag] ?? {});
  });
}

/// A language's own name for itself, such as “简体中文” or “English”.
function autonym(tag) {
  try {
    return new Intl.DisplayNames([tag], { type: "language" }).of(tag) ?? tag;
  } catch {
    return tag;
  }
}

/// A translation with `{name}` placeholders filled from `args`.
export function fmt(key, args) {
  return tr(key).replace(/\{\{|\}\}|\{([^{}]*)\}/g, (match, name) => {
    if (match === "{{") return "{";
    if (match === "}}") return "}";
    return name in args ? args[name] : "";
  });
}

/// Bern's basic, locale-independent number text.
export function num(value) {
  if (Number.isInteger(value) && Math.abs(value) < 1e15) return String(value);
  return String(value);
}

export function int(value) {
  return String(value);
}

// MARK: - Errors

const failures = cell(null);

function reportError(error) {
  console.error("[bern]", error);
}

class CommandFailure extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

// MARK: - WebAssembly host

/// The files the user gave the page (picked, dropped or opened with the
/// application), by the `blob:` URL the engine knows them by.
export const pageFiles = new Map();

/// The bytes of `blob`, read at once. The engine runs on the page's thread
/// and reads without waiting, and a synchronous request is the only way to
/// read a blob there; the bytes arrive as text decoded one byte per letter.
function readSync(blob) {
  const url = URL.createObjectURL(blob);
  try {
    const request = new XMLHttpRequest();
    request.open("GET", url, false);
    request.overrideMimeType("text/plain; charset=x-user-defined");
    request.send();
    if (request.status !== 200 && request.status !== 0) return null;
    const text = request.responseText;
    const bytes = new Uint8Array(text.length);
    for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i) & 0xff;
    return bytes;
  } catch {
    return null;
  } finally {
    URL.revokeObjectURL(url);
  }
}

class Host {
  constructor() {
    this.app = 0;
    this.replies = new Map();
    this.queries = new Map();
    this.nextToken = 1n;
    this.revision = 0n;
    this.timers = new Map();
  }

  async load(url) {
    const imports = {
      bern: {
        on_state: (ptr, len) => this.defer(this.copy(ptr, len), (bytes) => this.receiveState(bytes)),
        on_reply: (token, ptr, len) => this.defer(this.copy(ptr, len), (bytes) => this.receiveReply(token, bytes)),
        on_session_request: (request, ptr, len) =>
          this.defer(this.copy(ptr, len), (bytes) => sessionRequest(request, bytes)),
        on_media_request: (ptr, len) => this.defer(this.copy(ptr, len), (bytes) => media.request(bytes)),
        on_media_state: (ptr, len) => this.defer(this.copy(ptr, len), (bytes) => media.receive(bytes)),
        on_query: (subscription, ptr, len) =>
          this.defer(this.copy(ptr, len), (bytes) => this.queries.get(subscription)?.(bytes)),
        random_u32: () => crypto.getRandomValues(new Uint32Array(1))[0],
        // Files the page was given, which the engine reads.
        file_name: (ptr, len, out, capacity) => {
          const file = pageFiles.get(this.text(ptr, len));
          if (!file) return -1;
          const bytes = new TextEncoder().encode(file.name);
          if (bytes.length <= capacity) new Uint8Array(this.exports.memory.buffer, out, bytes.length).set(bytes);
          return bytes.length;
        },
        file_size: (ptr, len) => pageFiles.get(this.text(ptr, len))?.size ?? -1,
        file_read: (ptr, len, offset, out, count) => {
          const file = pageFiles.get(this.text(ptr, len));
          const bytes = file ? readSync(file.slice(offset, offset + count)) : null;
          if (!bytes) return -1;
          new Uint8Array(this.exports.memory.buffer, out, bytes.length).set(bytes);
          return bytes.length;
        },
        now_ms: () => performance.now(),
        set_timer: (id, delay) => {
          setTimeout(() => this.exports.bern_timer_fired(id), Math.max(0, delay));
        },
        // WebUI build: messages to the engine host and connection changes.
        link_send: (ptr, len) => link.send(new TextDecoder().decode(this.copy(ptr, len))),
        link_status: (ptr, len) => {
          const status = JSON.parse(new TextDecoder().decode(this.copy(ptr, len)));
          this.defer(null, () => link.status(status));
        },
      },
    };
    const response = await fetch(url);
    if (!response.ok) throw new Error(`${url}: ${response.status} ${response.statusText}`);
    const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), imports);
    this.exports = instance.exports;
    this.slot = this.exports.bern_bytes_alloc(16);
  }

  copy(ptr, len) {
    return new Uint8Array(this.exports.memory.buffer, ptr, len).slice();
  }

  text(ptr, len) {
    return new TextDecoder().decode(new Uint8Array(this.exports.memory.buffer, ptr, len));
  }

  /// Callbacks arrive while Rust is running; their work happens after the
  /// call that caused them returns, in arrival order.
  defer(bytes, handle) {
    queueMicrotask(() => {
      try {
        handle(bytes);
      } catch (error) {
        reportError(error);
      }
    });
  }

  /// Bytes into WebAssembly memory for the duration of `body`.
  withBytes(bytes, body) {
    const len = bytes.length;
    const ptr = this.exports.bern_bytes_alloc(len);
    new Uint8Array(this.exports.memory.buffer, ptr, len).set(bytes);
    try {
      return body(ptr, len);
    } finally {
      this.exports.bern_bytes_free(ptr, len);
    }
  }

  /// Call an export that returns a `BernBuffer` through a result slot.
  buffer(name, ...args) {
    this.exports[name](this.slot, ...args);
    const view = new DataView(this.exports.memory.buffer);
    const ptr = view.getUint32(this.slot, true);
    const len = view.getUint32(this.slot + 4, true);
    const bytes = new Uint8Array(this.exports.memory.buffer, ptr, len).slice();
    this.exports.bern_buffer_free(this.slot);
    return bytes;
  }

  /// The WebUI build: the page's runtime for an engine in an engine host.
  startRemote(token) {
    const bytes = new TextEncoder().encode(token);
    this.app = this.withBytes(bytes, (ptr, len) => this.exports.bern_webui_start(ptr, len));
  }

  remote(name, text) {
    const bytes = new TextEncoder().encode(text ?? "");
    this.withBytes(bytes, (ptr, len) => this.exports[name](this.app, ptr, len));
  }

  start(fingerprint, resources) {
    const path = new TextEncoder().encode(resources);
    const error = this.exports.bern_bytes_alloc(16);
    const app = this.withBytes(path, (ptr, len) => this.exports.bern_web_start(fingerprint, ptr, len, error));
    if (!app) {
      const view = new DataView(this.exports.memory.buffer);
      const message = new TextDecoder().decode(
        new Uint8Array(this.exports.memory.buffer, view.getUint32(error, true), view.getUint32(error + 4, true)),
      );
      this.exports.bern_buffer_free(error);
      throw new Error(message);
    }
    this.app = app;
    this.engineID = new TextDecoder().decode(this.buffer("bern_app_engine_id", app));
    this.receiveState(this.buffer("bern_app_snapshot", app));
  }

  receiveState(bytes) {
    const reader = new Reader(bytes);
    const revision = reader.u64();
    if (revision < this.revision) return;
    this.revision = revision;
    const count = reader.u32();
    batch(() => {
      for (let i = 0; i < count; i++) engine.apply(reader.u32(), reader);
    });
  }

  dispatch(command, encode, window, completion) {
    const writer = new Writer();
    encode(writer);
    const token = this.nextToken++;
    this.replies.set(token, { window, completion });
    this.withBytes(writer.finish(), (ptr, len) => this.exports.bern_app_dispatch(this.app, command, ptr, len, token));
  }

  receiveReply(token, bytes) {
    const pending = this.replies.get(token);
    if (!pending) return;
    this.replies.delete(token);
    const reader = new Reader(bytes);
    const status = reader.u8();
    reader.u64();
    if (status === 0) {
      pending.completion?.(null);
    } else {
      const failure = new CommandFailure(reader.text(), reader.text());
      pending.window?.fail(failure);
      pending.completion?.(failure);
    }
  }

  /// `0` or `1, code, message`.
  status(bytes) {
    const reader = new Reader(bytes);
    if (reader.u8() === 0) return null;
    return new CommandFailure(reader.text(), reader.text());
  }

  windowOpened(kind) {
    return this.buffer("bern_window_opened", this.app, kind);
  }

  windowClosed(kind, generation) {
    this.exports.bern_window_closed(this.app, kind, generation);
  }

  windowFacts(win, facts) {
    this.exports.bern_window_facts(
      this.app, win.kind, win.generation, win.commandInFlight ? 0 : 1, facts.visibility, facts.size,
      facts.x, facts.y, facts.width, facts.height, facts.focused ? 1 : 0, facts.actions,
    );
  }

  sessionSet(win, session, field, bytes) {
    if (!win.generation) return false;
    const result = this.withBytes(bytes, (ptr, len) =>
      this.buffer("bern_session_set", this.app, win.kind, win.generation, session, field, ptr, len));
    const failure = this.status(result);
    if (failure) reportError(`${failure.code}: ${failure.message}`);
    return !failure;
  }

  requestClose(win, needsConfirmation, canSave) {
    return new Reader(this.buffer(
      "bern_window_request_close", this.app, win.kind, win.generation, needsConfirmation ? 1 : 0, canSave ? 1 : 0,
    ));
  }

  confirmClose(win, confirmation, decision) {
    const writer = new Writer();
    writer.text(confirmation);
    writer.u32(decision);
    return new Reader(this.withBytes(writer.finish(), (ptr, len) =>
      this.buffer("bern_window_confirm_close", this.app, win.kind, win.generation, ptr, len)));
  }

  queryOpen(win, index, receive) {
    const reader = new Reader(this.buffer("bern_query_open", this.app, win.kind, win.generation, index));
    if (reader.u8() !== 0) throw new CommandFailure(reader.text(), reader.text());
    const subscription = reader.u64();
    this.queries.set(subscription, receive);
    return subscription;
  }

  queryRequest(subscription, params) {
    const bytes = this.withBytes(params, (ptr, len) =>
      this.buffer("bern_query_request", this.app, subscription, ptr, len));
    const reader = new Reader(bytes);
    if (reader.u8() !== 0) throw new CommandFailure(reader.text(), reader.text());
    return bytes.subarray(1);
  }

  queryClose(subscription) {
    this.queries.delete(subscription);
    this.exports.bern_query_close(this.app, subscription);
  }

  sessionComplete(request, bytes) {
    this.withBytes(bytes, (ptr, len) => this.exports.bern_session_complete(this.app, request, ptr, len));
  }

  /// A media command; failures show in `window`.
  dispatchMedia(command, encode, window, completion) {
    const writer = new Writer();
    encode(writer);
    const token = this.nextToken++;
    this.replies.set(token, { window, completion });
    this.withBytes(writer.finish(), (ptr, len) => this.exports.bern_media_dispatch(this.app, command, ptr, len, token));
  }

  /// A media call that answers `0…` or `1, code, message`.
  mediaCall(name, bytes) {
    return this.withBytes(bytes, (ptr, len) => this.buffer(name, this.app, ptr, len));
  }

  mediaPost(name, bytes) {
    this.withBytes(bytes, (ptr, len) => this.exports[name](this.app, ptr, len));
  }
}

const host = new Host();

/// The generated engine mirror: `apply(field, reader)` and its cells.
let engine = { apply() {} };

// MARK: - Session requests

/// Run a session command of an external entry through the same closures
/// the user's input runs, in the order of the framework session catalog.
function sessionRequest(request, bytes) {
  const reader = new Reader(bytes);
  const index = reader.u32();
  const ok = (encode) => {
    const writer = new Writer();
    writer.u8(0);
    writer.u32(index);
    encode?.(writer);
    host.sessionComplete(request, writer.finish());
  };
  const fail = (failure) => {
    const writer = new Writer();
    writer.u8(1);
    writer.text(failure?.code ?? "ui_failed");
    writer.text(failure?.message ?? String(failure));
    host.sessionComplete(request, writer.finish());
  };
  const find = (targetName) => {
    const matches = controlEntries().filter((entry) => entry.target() === targetName);
    if (matches.length === 0) throw new CommandFailure("target_not_found", `no mounted control \`${targetName}\`; see ui.inspect`);
    if (matches.length > 1) throw new CommandFailure("target_ambiguous", `\`${targetName}\` is mounted in several places`);
    const entry = matches[0];
    if (entry.window.pendingClose.peek()) {
      throw new CommandFailure("target_blocked", `\`${targetName}\` is blocked by the close confirmation of \`${entry.window.id}\`; decide it with window.confirm_close`);
    }
    if (entry.enabled && !entry.enabled()) throw new CommandFailure("target_unavailable", `\`${targetName}\` is disabled`);
    return entry;
  };
  const action = (entry, name) => {
    const run = entry.actions[name];
    if (!run) throw new CommandFailure("target_unavailable", `\`${entry.target()}\` (${entry.role}) does not support ${name}`);
    return run;
  };
  const windowNamed = (id) => {
    const win = [...manager.windows.values()].find((candidate) => candidate.id === id);
    if (!win) throw new CommandFailure("target_not_found", `no open window \`${id}\``);
    return win;
  };
  const closeResult = (writer, result) => {
    writer.u32(result.outcome);
    writer.text(result.window);
    writeOptional(writer, result.id ?? null, (w, v) => w.text(v));
    writer.i64(BigInt(result.draftRevision ?? 0n));
    const decisions = result.outcome === 2 ? (result.canSave ? [0, 1, 2] : [1, 2]) : [];
    writer.u32(decisions.length);
    for (const decision of decisions) writer.u32(decision);
    writeOptional(writer, result.message ?? null, (w, v) => w.text(v));
    writeOptional(writer, result.reason ?? null, (w, v) => w.text(v));
  };
  try {
    switch (index) {
      case 0: { // ui.inspect
        const entries = controlEntries();
        ok((writer) => {
          writer.u32(entries.length);
          for (const entry of entries) {
            const enabled = entry.enabled ? untrack(entry.enabled) : true;
            writer.text(entry.target());
            writer.text(entry.window.id);
            writer.text(entry.role);
            writer.text(entry.label ? untrack(entry.label) : "");
            writer.u8(enabled ? 1 : 0);
            writeOptional(writer, entry.actions.value ? untrack(entry.actions.value) : null, (w, v) => w.text(v));
            const reason = entry.window.pendingClose.peek() ? "blocked_by_modal" : enabled ? null : "disabled";
            writeOptional(writer, reason, (w, v) => w.text(v));
          }
        });
        break;
      }
      case 1: action(find(reader.text()), "activate")(); ok(); break;
      case 2: { const entry = find(reader.text()); action(entry, "edit")(reader.text()); ok(); break; }
      case 3: action(find(reader.text()), "submit")(); ok(); break;
      case 4: { const entry = find(reader.text()); action(entry, "setBool")(reader.bool()); ok(); break; }
      case 5: { const entry = find(reader.text()); action(entry, "setNumber")(reader.f64()); ok(); break; }
      case 6: {
        const entry = find(reader.text());
        const option = reader.text();
        if (!action(entry, "select")(option)) throw new CommandFailure("invalid_params", `\`${entry.target()}\` has no option \`${option}\``);
        ok();
        break;
      }
      case 7: {
        if (!action(find(reader.text()), "back")()) throw new CommandFailure("target_unavailable", "nothing to go back to");
        ok();
        break;
      }
      case 8: { // window.open
        const id = reader.text();
        if (!manager.app.windows.some((kind) => kind.id === id)) throw new CommandFailure("target_not_found", `no window kind \`${id}\``);
        const win = manager.open(id);
        ok((writer) => writer.text(win.id));
        break;
      }
      case 9: { // window.close
        const win = windowNamed(reader.text());
        const result = manager.requestClose(win);
        ok((writer) => closeResult(writer, result));
        break;
      }
      case 10: manager.open(windowNamed(reader.text()).spec.id); ok(); break;
      case 11: { // window.confirm_close
        const win = windowNamed(reader.text());
        const confirmation = reader.text();
        const decision = reader.u32();
        if (decision > 2) throw new CommandFailure("invalid_params", "unknown close decision");
        manager.confirmClose(win, confirmation, decision, (failure, result) => {
          if (failure) fail(failure);
          else ok((writer) => closeResult(writer, result));
        });
        break;
      }
      case 13: manager.command(windowNamed(reader.text()), "minimize"); ok(); break;
      case 14: manager.command(windowNamed(reader.text()), "restore"); ok(); break;
      case 15: manager.command(windowNamed(reader.text()), "maximize"); ok(); break;
      case 16: {
        const win = windowNamed(reader.text());
        manager.command(win, "move", { x: reader.f64(), y: reader.f64() });
        ok();
        break;
      }
      case 17: {
        const win = windowNamed(reader.text());
        const width = reader.f64();
        const height = reader.f64();
        const minimum = { width: win.spec.minWidth ?? 240, height: win.spec.minHeight ?? 160 };
        if (width < minimum.width || height < minimum.height) {
          throw new CommandFailure("out_of_range", `\`${win.id}\` needs at least ${minimum.width} × ${minimum.height}`);
        }
        manager.command(win, "resize", { width, height });
        ok();
        break;
      }
      case 18: ok((writer) => writer.text(languageCell.peek())); break; // ui.language
      case 19: setLanguage(reader.text()); ok(); break; // ui.set_language
      case 20: action(find(reader.text()), "focus")(); ok(); break; // ui.focus
      case 21: { // ui.set_file
        const entry = find(reader.text());
        const url = reader.text();
        const give = action(entry, "setFile");
        try {
          new URL(url);
        } catch {
          throw new CommandFailure("invalid_url", `\`${url}\` is not a URL`);
        }
        if (hooks.accepts && !hooks.accepts(url, entry.actions.fileKinds ?? "any")) {
          throw new CommandFailure("unsupported_file", `\`${entry.target()}\` takes ${entry.actions.fileKinds}, not \`${url}\``);
        }
        give(url);
        ok();
        break;
      }
      default:
        throw new CommandFailure("unknown_command", `unknown session command ${index}`);
    }
  } catch (error) {
    fail(error);
  }
}

// MARK: - Engine host link

/// The WebUI build's channel to its engine host: one WebSocket, opened
/// again after a drop. Waiting requests fail with an unknown outcome when
/// it drops; nothing is sent again by itself.
const link = {
  socket: null,
  attempts: 0,
  engineID: null,
  sessionID: null,
  connected: cell(false),
  problem: cell(null),
  firstConnect: null,

  open() {
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    const socket = new WebSocket(`${scheme}://${location.host}/bern`);
    this.socket = socket;
    socket.addEventListener("open", () => {
      this.attempts = 0;
      host.exports.bern_remote_open(host.app);
    });
    socket.addEventListener("message", (event) => host.remote("bern_remote_receive", String(event.data)));
    socket.addEventListener("close", (event) => {
      if (this.socket !== socket) return;
      this.socket = null;
      host.remote("bern_remote_closed", event.reason || `code ${event.code}`);
      if (this.problem.peek()?.final) return;
      const delay = Math.min(10000, 500 * 2 ** this.attempts++);
      setTimeout(() => this.open(), delay);
    });
  },

  send(text) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(text);
  },

  status(status) {
    if (status.state === "connected") {
      if (this.engineID && status.engine_id !== this.engineID) {
        // The engine host restarted: this page's session ended with it.
        this.problem.set({ key: "bern.link.restarted", final: true });
        setTimeout(() => location.reload(), 1500);
        return;
      }
      this.engineID = status.engine_id;
      this.sessionID = status.session_id;
      this.problem.set(null);
      this.connected.set(true);
      this.firstConnect?.resolve();
      this.firstConnect = null;
    } else if (status.state === "disconnected") {
      this.connected.set(false);
      if (!this.problem.peek()?.final) this.problem.set({ key: "bern.link.lost" });
    } else if (status.state === "failed") {
      this.connected.set(false);
      this.problem.set({ key: "bern.link.failed", detail: status.message, final: true });
      this.socket?.close();
      this.firstConnect?.reject(new Error(status.message));
      this.firstConnect = null;
    }
  },
};

/// The banner telling the user the engine host is out of reach.
function linkBanner(root) {
  const banner = el("div", { class: "bern-link-banner", role: "status" });
  effect(() => {
    const problem = link.problem.get();
    banner.hidden = !problem;
    if (!problem) return;
    banner.replaceChildren(icon("exclamationmark.triangle", 14), el("span", {}, tr(problem.key)));
    if (problem.detail) banner.append(el("span", { class: "bern-link-detail" }, problem.detail));
  });
  root.prepend(banner);
}

/// Player ids are unique per engine host: qualify those of window
/// declarations by session in the WebUI build.
function sessionPrefix() {
  return link.sessionID ? `${link.sessionID}/` : "";
}

// MARK: - Hooks

/// Parts of the page `features.js` provides: styles, structural motion and
/// the like. It fills these in when it loads, before anything is built.
export const hooks = {
  /// `(node, style, { selected })`: a `style!` override on a control.
  style: null,
  /// `(node, done)`: plays the disappearing transition of `node`, then
  /// calls `done`; false when it has none.
  exit: null,
  /// `(win, seconds)`: keeps `win.idle` from the window's input.
  idle: null,
  /// `(node, property, value, motion)`: a length bound to state.
  length: null,
  /// `(url, kinds)`: whether a file at `url` is one of `kinds`.
  accepts: null,
  /// The stagger clock of the container being built, if any.
  clock: null,
  /// `(step)`: a new stagger clock.
  makeClock: null,
  /// `(clock, body)`: runs `body` with `clock` as the current one.
  withClock: null,
};

// MARK: - DOM

const SVG = "http://www.w3.org/2000/svg";
let icons = {};

export function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attrs)) {
    if (value === null || value === undefined || value === false) continue;
    if (name === "class") node.className = value;
    else if (name.startsWith("on")) node.addEventListener(name.slice(2), value);
    else node.setAttribute(name, value === true ? "" : value);
  }
  for (const child of children) if (child) node.append(child);
  return node;
}

let iconSequence = 0;

/// A Lucide icon: the shapes inherit Lucide's stroke from the root, and a
/// knocked-out icon's mask gets an id of its own.
export function icon(name, size = 16) {
  const svg = document.createElementNS(SVG, "svg");
  for (const [attribute, value] of [
    ["viewBox", "0 0 24 24"], ["width", size], ["height", size], ["fill", "none"], ["stroke", "currentColor"],
    ["stroke-width", "2"], ["stroke-linecap", "round"], ["stroke-linejoin", "round"], ["aria-hidden", "true"],
    ["class", "bern-icon"],
  ]) svg.setAttribute(attribute, value);
  svg.innerHTML = (icons[name] ?? icons.circle ?? "").replaceAll("__ID__", `bern-icon-${++iconSequence}`);
  return svg;
}

/// Keep a text node, attribute or property in step with a getter.
export function bindText(node, getter) {
  effect(() => {
    node.textContent = getter();
  });
}

export function bindAttr(node, name, getter) {
  effect(() => {
    const value = getter();
    if (value === null || value === undefined || value === false) node.removeAttribute(name);
    else node.setAttribute(name, value === true ? "" : value);
  });
}

/// Whether elements mounting now play their `appear` transition: true
/// while `When` or `Each` change after their first display, for the
/// elements they mount themselves.
let entering = false;
/// Whether an update after the first display is mounting elements, at any
/// depth: containers that stagger their children play them then.
let updating = false;

export function structural(animated, body) {
  const saved = [entering, updating];
  entering = animated;
  updating = updating || animated;
  try {
    return body();
  } finally {
    [entering, updating] = saved;
  }
}

export function isEntering() {
  return entering;
}

export function isUpdating() {
  return updating;
}

/// Runs `body` with the stagger clock of a container, when there is one.
function withClock(clock, body) {
  return clock && hooks.withClock ? hooks.withClock(clock, body) : body();
}

/// Take `node` off the page, once its disappearing transition has played.
function leave(node) {
  if (node.bernExiting) return;
  if (!(node.nodeType === 1 && hooks.exit?.(node, () => node.remove()))) node.remove();
}

/// A region between two markers whose contents a builder replaces.
class Region {
  constructor(parent) {
    this.start = document.createComment("");
    this.end = document.createComment("");
    parent.append(this.start, this.end);
  }
  clear() {
    const nodes = [];
    for (let node = this.start.nextSibling; node && node !== this.end; node = node.nextSibling) nodes.push(node);
    for (const node of nodes) leave(node);
  }
  /// A detached fragment the builder fills, then inserted at once.
  fill(build) {
    const fragment = document.createDocumentFragment();
    build(fragment);
    this.end.before(fragment);
  }
}

// MARK: - Controls

/// Mounted controls by target; external entries operate them through the
/// same closures the user's input runs.
const controls = new Map();
let controlSequence = 0;

export function control(ctx, entry) {
  const token = ++controlSequence;
  const record = { ...entry, window: ctx.win };
  controls.set(token, record);
  onCleanup(() => controls.delete(token));
  return record;
}

export function controlEntries() {
  return [...controls.values()];
}

export function target(ctx, id) {
  const base = `${ctx.comp}.${id}`;
  // The key of every repeated row the control is in, outermost first.
  return base + (ctx.keys ?? []).map((key) => `[${key()}]`).join("");
}

export function mark(node, ctx, id) {
  effect(() => node.setAttribute("data-bern-target", target(ctx, id)));
}

// MARK: - Contexts

/// What a component body reads: its window, the window's sessions, the
/// component name for control targets, the row key inside lists, the page
/// whose header shows titles and tools, and the enclosing navigation path.
export function child(ctx, comp) {
  return { ...ctx, comp };
}

export function page() {
  return {
    titles: cell([]), backs: cell([]), toolCount: cell(0),
    tools: el("div", { class: "bern-tools", role: "toolbar" }),
  };
}

function stackPush(stack, entry) {
  stack.set([...stack.peek(), entry]);
  onCleanup(() => stack.set(stack.peek().filter((item) => item !== entry)));
}

/// A title for the enclosing page; the innermost registered one wins.
export function pageTitle(ctx, getter) {
  if (ctx.page) stackPush(ctx.page.titles, getter);
}

/// Toolbar items for the enclosing page header.
export function tools(ctx, build) {
  if (!ctx.page) return;
  const holder = el("div", { class: "bern-tool-group" });
  ctx.page.tools.append(holder);
  const count = ctx.page.toolCount;
  count.set(count.peek() + 1);
  onCleanup(() => {
    holder.remove();
    count.set(count.peek() - 1);
  });
  build(holder, { ...ctx, inToolbar: true });
}

export function pageHeader(ctx, fallback, always) {
  const back = el("button", { class: "bern-icon-button bern-back", type: "button" }, icon("chevron.left", 18));
  bindAttr(back, "aria-label", () => tr("bern.navigation.back"));
  back.addEventListener("click", () => {
    const backs = ctx.page.backs.peek();
    backs[backs.length - 1]?.();
  });
  const title = el("h2", { class: "bern-page-title" });
  const header = el("header", { class: "bern-page-header" }, back, title, ctx.page.tools);
  effect(() => {
    const titles = ctx.page.titles.get();
    const text = titles.length ? titles[titles.length - 1]() : fallback ? fallback() : "";
    title.textContent = text;
    const backs = ctx.page.backs.get();
    back.hidden = backs.length === 0;
    header.hidden = !always && !text && backs.length === 0 && ctx.page.toolCount.get() === 0;
  });
  return header;
}

// MARK: - Components

export const STYLE_CLASS = {
  large_title: "large-title", title: "title", title2: "title2", title3: "title3", headline: "headline",
  subheadline: "subheadline", body: "body", callout: "callout", footnote: "footnote", caption: "caption",
  caption2: "caption2",
};

export function Text(p, ctx, { content, style, emphasis }) {
  const node = el("p", { class: `bern-text bern-${STYLE_CLASS[style] ?? "body"} bern-emphasis-${emphasis ?? "primary"}` });
  bindText(node, content);
  p.append(node);
}

export function Label(p, ctx, { title, icon: name, style }) {
  const text = el("span");
  bindText(text, title);
  p.append(el("span", { class: `bern-label bern-${STYLE_CLASS[style] ?? "body"}` }, icon(name), text));
}

function enabledOf(enabled) {
  return enabled ?? (() => true);
}

export function Button(p, ctx, { id, label, icon: name, role, prominence, enabled, style, setFile, fileKinds }, action) {
  const kind = prominence ?? "standard";
  const text = el("span");
  bindText(text, label);
  const node = el(
    "button",
    { class: `bern-button bern-button-${kind}${role === "destructive" ? " bern-destructive" : ""}`, type: "button" },
    name ? icon(name, 14) : null,
    text,
  );
  const isEnabled = enabledOf(enabled);
  effect(() => {
    node.disabled = !isEnabled();
  });
  mark(node, ctx, id);
  node.addEventListener("click", () => action());
  control(ctx, {
    target: () => target(ctx, id), role: "button", label, enabled: isEnabled,
    actions: { activate: action, ...(setFile ? { setFile, fileKinds } : {}) },
  });
  if (style) hooks.style?.(node, style, {});
  p.append(ctx.inGroup && !ctx.inToolbar ? el("div", { class: "bern-row bern-row-button" }, node) : node);
}

export function IconButton(p, ctx, { id, icon: name, label, enabled, style, setFile, fileKinds }, action) {
  const node = el("button", { class: "bern-icon-button", type: "button" }, icon(name, 16));
  bindAttr(node, "aria-label", label);
  bindAttr(node, "title", label);
  const isEnabled = enabledOf(enabled);
  effect(() => {
    node.disabled = !isEnabled();
  });
  mark(node, ctx, id);
  node.addEventListener("click", () => action());
  control(ctx, {
    target: () => target(ctx, id), role: "button", label, enabled: isEnabled,
    actions: { activate: action, ...(setFile ? { setFile, fileKinds } : {}) },
  });
  if (style) hooks.style?.(node, style, {});
  p.append(node);
}

let fieldSequence = 0;

/// A form row: the label, then `display`, whose focusable part `input`
/// the label names.
function labelled(display, label, stacked, input = display) {
  const id = `bern-field-${++fieldSequence}`;
  input.id = id;
  const caption = el("label", { class: "bern-row-label", for: id });
  bindText(caption, label);
  return el("div", { class: `bern-row${stacked ? " bern-row-stacked" : ""}` }, caption, display);
}

/// Text editing: input events update the draft except during input method
/// composition, which writes once when it ends.
function editable(input, field) {
  let composing = false;
  input.addEventListener("compositionstart", () => {
    composing = true;
  });
  input.addEventListener("compositionend", () => {
    composing = false;
    field.set(input.value);
  });
  input.addEventListener("input", () => {
    if (!composing) field.set(input.value);
  });
  effect(() => {
    const value = field.get();
    if (!composing && input.value !== value) input.value = value;
  });
}

export function TextInput(p, ctx, { id, label, field, prompt, kind }, submit) {
  const input = kind === "multiline"
    ? el("textarea", { class: "bern-field", rows: 3 })
    : el("input", { class: "bern-field", type: kind === "secure" ? "password" : "text", autocomplete: "off" });
  if (prompt) bindAttr(input, "placeholder", prompt);
  editable(input, field);
  if (submit && kind !== "multiline") {
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.isComposing) submit();
    });
  }
  mark(input, ctx, id);
  control(ctx, {
    target: () => target(ctx, id), role: "text_input", label,
    actions: { edit: (value) => field.set(value), submit, value: kind === "secure" ? null : () => field.peek() },
  });
  p.append(labelled(input, label, true));
}

export function Search(p, ctx, { id, field, prompt }, submit, results) {
  const input = el("input", { class: "bern-search-input", type: "search", autocomplete: "off" });
  if (prompt) {
    bindAttr(input, "placeholder", prompt);
    bindAttr(input, "aria-label", prompt);
  }
  editable(input, field);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.isComposing) submit?.();
    if (event.key === "Escape" && field.peek() !== "") {
      event.preventDefault();
      field.set("");
    }
  });
  const clear = el("button", { class: "bern-search-clear", type: "button", tabindex: "-1" }, icon("xmark.circle.fill", 14));
  bindAttr(clear, "aria-label", () => tr("bern.search.clear"));
  clear.addEventListener("click", () => {
    field.set("");
    input.focus();
  });
  effect(() => {
    clear.hidden = field.get() === "";
  });
  mark(input, ctx, id);
  control(ctx, {
    target: () => target(ctx, id), role: "search", label: prompt ?? (() => ""),
    actions: { edit: (value) => field.set(value), submit, value: () => field.peek(), focus: () => input.focus() },
  });
  const box = el("div", { class: "bern-search" }, icon("magnifyingglass", 14), input, clear);
  if (results) searchResults(box, input, field, ctx, results);
  p.append(box);
}

/// Results of a search field in a popover below it, shown while the field
/// has focus and text; choosing a result moves focus into the panel, which
/// keeps it open until focus leaves both.
function searchResults(box, input, field, ctx, results) {
  const panel = el("div", { class: "bern-search-results", popover: "manual", role: "region" });
  results(panel, ctx);
  box.append(panel);
  const open = cell(false);
  // Choosing a result closes the list.
  panel.addEventListener("click", (event) => {
    if (event.target.closest("button, [role=button]")) open.set(false);
  });
  const inside = () => box.contains(document.activeElement);
  const update = () => open.set(inside() && field.peek() !== "");
  input.addEventListener("focus", update);
  input.addEventListener("input", update);
  box.addEventListener("focusout", () => setTimeout(update, 120));
  input.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && open.peek()) {
      event.preventDefault();
      open.set(false);
    }
  });
  effect(() => {
    field.get();
    untrack(update);
  });
  effect(() => {
    const show = open.get();
    if (show && !panel.matches(":popover-open")) {
      placePopover(panel, box);
      // At least 280 wide, and within the window.
      const rect = box.getBoundingClientRect();
      const width = Math.min(Math.max(rect.width, 280), innerWidth - 16);
      panel.style.minWidth = `${width}px`;
      panel.style.left = `${Math.max(8, Math.min(rect.left, innerWidth - width - 8))}px`;
      panel.showPopover();
    } else if (!show && panel.matches(":popover-open")) {
      panel.hidePopover();
    }
  });
  onCleanup(() => {
    if (panel.matches(":popover-open")) panel.hidePopover();
  });
}

export function Switch(p, ctx, { id, label, field, style }) {
  let node;
  if (style === "checkbox") {
    node = el("button", { class: "bern-checkbox", type: "button", role: "checkbox" }, icon("checkmark", 12));
  } else {
    node = el("button", { class: "bern-switch", type: "button", role: "switch" }, el("span", { class: "bern-switch-thumb" }));
  }
  effect(() => node.setAttribute("aria-checked", field.get() ? "true" : "false"));
  node.addEventListener("click", () => field.set(!field.peek()));
  mark(node, ctx, id);
  control(ctx, {
    target: () => target(ctx, id), role: "switch", label,
    actions: { setBool: (value) => field.set(value), value: () => (field.peek() ? "true" : "false") },
  });
  p.append(labelled(node, label, false));
}

export function Slider(p, ctx, { id, label, field, min, max, step, minIcon, maxIcon }, commit) {
  const input = el("input", { class: "bern-slider", type: "range", min, max, step: step ?? "any" });
  const fill = () => {
    const ratio = max > min ? (Number(input.value) - min) / (max - min) : 0;
    input.style.setProperty("--bern-fill", `${Math.min(1, Math.max(0, ratio)) * 100}%`);
  };
  effect(() => {
    const value = field.get();
    if (Number(input.value) !== value) input.value = value;
    fill();
  });
  input.addEventListener("input", () => {
    field.set(Number(input.value));
    fill();
  });
  input.addEventListener("change", () => commit?.());
  mark(input, ctx, id);
  control(ctx, {
    target: () => target(ctx, id), role: "slider", label,
    actions: {
      setNumber: (value) => {
        field.set(value);
        commit?.();
      },
      value: () => num(field.peek()),
    },
  });
  const track = el("div", { class: "bern-slider-track" }, minIcon ? icon(minIcon, 14) : null, input, maxIcon ? icon(maxIcon, 14) : null);
  p.append(labelled(track, label, true, input));
}

let typeaheadReset = 0;

/// Place a popover below `anchor`, or above it when there is more room
/// there. It runs before the popover shows, from the anchor alone: the
/// popover's height is capped by the room it gets.
export function placePopover(popover, anchor) {
  const rect = anchor.getBoundingClientRect();
  const below = innerHeight - rect.bottom - 12;
  const above = rect.top - 12;
  const up = below < 160 && above > below;
  popover.style.minWidth = `${rect.width}px`;
  popover.style.maxWidth = `${Math.max(rect.width, innerWidth - rect.left - 8)}px`;
  popover.style.left = `${Math.max(8, Math.min(rect.left, innerWidth - 168))}px`;
  popover.style.top = up ? "auto" : `${rect.bottom + 4}px`;
  popover.style.bottom = up ? `${innerHeight - rect.top + 4}px` : "auto";
  popover.style.maxHeight = `${Math.min(328, up ? above : below)}px`;
}

/// Bern's own dropdown, a select-only combobox (WAI-ARIA APG). The list is
/// a popover in the browser's top layer, so cards, dialogs and scroll
/// containers never clip it. Focus stays on the trigger; the active option
/// is announced through `aria-activedescendant`. Choosing runs `select`,
/// the same closure touch and external entries use.
function dropdown(options, field, select) {
  const listId = `bern-listbox-${++fieldSequence}`;
  const value = el("span", { class: "bern-dropdown-value" });
  const trigger = el("button", {
    class: "bern-dropdown", type: "button", role: "combobox", "aria-haspopup": "listbox",
    "aria-expanded": "false", "aria-controls": listId, popovertarget: listId,
  }, value, icon("chevron.down", 16));
  const list = el("div", { class: "bern-menu", role: "listbox", id: listId, popover: "auto", tabindex: "-1" });
  let active = -1;
  // Keep the active option visible inside the list only.
  const reveal = () => {
    const item = items[active];
    if (!item) return;
    if (item.offsetTop < list.scrollTop) list.scrollTop = item.offsetTop - 4;
    else if (item.offsetTop + item.offsetHeight > list.scrollTop + list.clientHeight) {
      list.scrollTop = item.offsetTop + item.offsetHeight - list.clientHeight + 4;
    }
  };
  const setActive = (index) => {
    active = index;
    items.forEach((item, at) => item.classList.toggle("bern-menu-active", at === index));
    if (index >= 0) {
      trigger.setAttribute("aria-activedescendant", items[index].id);
      reveal();
    } else trigger.removeAttribute("aria-activedescendant");
  };
  const isOpen = () => list.matches(":popover-open");
  const selected = () => options.findIndex((option) => eq(option.value, field.peek()));
  const close = () => {
    if (isOpen()) list.hidePopover();
  };
  const choose = (index, refocus = true) => {
    if (index >= 0) select(options[index].name);
    close();
    if (refocus) trigger.focus();
  };
  const items = options.map((option, index) => {
    const text = el("span", { class: "bern-menu-text" });
    bindText(text, option.label);
    const check = icon("checkmark", 16);
    check.classList.add("bern-menu-check");
    const item = el("div", { class: "bern-menu-item", role: "option", id: `${listId}-${index}` }, text, check);
    item.addEventListener("pointermove", () => {
      if (active !== index) setActive(index);
    });
    item.addEventListener("click", () => choose(index));
    list.append(item);
    return item;
  });
  const place = () => placePopover(list, trigger);
  const dismiss = (event) => {
    if (!list.contains(event.target)) close();
  };
  list.addEventListener("beforetoggle", (event) => {
    const open = event.newState === "open";
    trigger.setAttribute("aria-expanded", open ? "true" : "false");
    if (open) {
      place();
      setActive(Math.max(0, selected()));
      addEventListener("scroll", dismiss, true);
      addEventListener("resize", close);
    } else {
      setActive(-1);
      removeEventListener("scroll", dismiss, true);
      removeEventListener("resize", close);
    }
  });
  list.addEventListener("toggle", (event) => {
    if (event.newState === "open") reveal();
  });
  // Typing jumps to the next option whose label starts with the typed text.
  let typed = "";
  const typeahead = (key) => {
    clearTimeout(typeaheadReset);
    typeaheadReset = setTimeout(() => {
      typed = "";
    }, 500);
    typed += key.toLowerCase();
    const start = typed.length === 1 ? active + 1 : Math.max(active, 0);
    for (let step = 0; step < options.length; step++) {
      const index = (start + step) % options.length;
      if (untrack(options[index].label).toLowerCase().startsWith(typed)) return index;
    }
    return -1;
  };
  trigger.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    const key = event.key;
    const printable = key.length === 1 && key !== " " && !event.ctrlKey && !event.metaKey && !event.altKey;
    if (!isOpen()) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(key) || printable) {
        event.preventDefault();
        list.showPopover();
        if (printable) {
          const found = typeahead(key);
          if (found >= 0) setActive(found);
        }
      }
      return;
    }
    const last = options.length - 1;
    const moves = { ArrowDown: Math.min(last, active + 1), ArrowUp: Math.max(0, active - 1), Home: 0, End: last };
    if (key in moves && !(event.altKey && key === "ArrowUp")) {
      event.preventDefault();
      setActive(moves[key]);
    } else if (key === "Enter" || key === " " || (event.altKey && key === "ArrowUp")) {
      event.preventDefault();
      choose(active);
    } else if (key === "Escape") {
      // Handled here, so an enclosing dialog does not also close.
      event.preventDefault();
      close();
    } else if (key === "Tab") {
      choose(active, false);
    } else if (printable) {
      event.preventDefault();
      const found = typeahead(key);
      if (found >= 0) setActive(found);
    }
  });
  effect(() => {
    const current = field.get();
    const index = options.findIndex((option) => eq(option.value, current));
    value.textContent = index >= 0 ? options[index].label() : "";
    items.forEach((item, at) => item.setAttribute("aria-selected", at === index ? "true" : "false"));
  });
  onCleanup(close);
  const node = el("div", { class: "bern-dropdown-wrap" }, trigger, list);
  node.control = trigger;
  return node;
}

export function Picker(p, ctx, { id, label, field, options, style }) {
  let node;
  const select = (name) => {
    const option = options.find((candidate) => candidate.name === name);
    if (!option) return false;
    field.set(option.value);
    return true;
  };
  if (style === "segmented" || style === "radio" || style === "inline") {
    node = el("div", { class: style === "segmented" ? "bern-segmented" : "bern-radio-group", role: "radiogroup" });
    bindAttr(node, "aria-label", label);
    const buttons = options.map((option) => {
      const text = el("span");
      bindText(text, option.label);
      const button = el(
        "button",
        { class: style === "segmented" ? "bern-segment" : "bern-radio", type: "button", role: "radio" },
        style === "segmented" ? null : el("span", { class: "bern-radio-mark" }),
        text,
      );
      button.addEventListener("click", () => field.set(option.value));
      button.addEventListener("keydown", (event) => {
        const index = options.indexOf(option);
        const next = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
        if (next) {
          event.preventDefault();
          const target = options[(index + next + options.length) % options.length];
          field.set(target.value);
          buttons[options.indexOf(target)].focus();
        }
      });
      node.append(button);
      return button;
    });
    effect(() => {
      const value = field.get();
      options.forEach((option, index) => {
        const selected = eq(option.value, value);
        buttons[index].setAttribute("aria-checked", selected ? "true" : "false");
        buttons[index].tabIndex = selected ? 0 : -1;
      });
    });
  } else {
    node = dropdown(options, field, select);
  }
  mark(node.control ?? node, ctx, id);
  control(ctx, {
    target: () => target(ctx, id), role: "picker", label,
    actions: { select, value: () => options.find((option) => eq(option.value, field.peek()))?.name ?? null },
  });
  if (ctx.inToolbar) p.append(node);
  else p.append(labelled(node, label, style === "radio" || style === "inline", node.control ?? node));
}

export function Stack(p, ctx, { axis, gap, padding, align }, children) {
  const node = el("div", {
    class: `bern-stack bern-${axis === "column" ? "vstack" : "hstack"} bern-align-${align ?? (axis === "column" ? "leading" : "center")}`,
  });
  if (gap !== undefined) node.style.gap = `${gap}px`;
  if (padding !== undefined) node.style.padding = `${padding}px`;
  children(node, { ...ctx, inGroup: false });
  p.append(node);
}

/// Equal columns, as many as fit at `minWidth` or wider.
export function Grid(p, ctx, { minWidth, gap, padding }, children) {
  const node = el("div", { class: "bern-grid bern-fill-width" });
  node.style.gridTemplateColumns = `repeat(auto-fill, minmax(min(100%, ${minWidth}px), 1fr))`;
  node.style.gap = `${gap ?? 8}px`;
  if (padding !== undefined) node.style.padding = `${padding}px`;
  children(node, { ...ctx, inGroup: false });
  p.append(node);
}

/// Where the layers of a `Stack` sit: `align-items` then `justify-items`.
const LAYER_PLACE = {
  top_leading: "start start", top: "start center", top_trailing: "start end",
  leading: "center start", center: "center center", trailing: "center end",
  bottom_leading: "end start", bottom: "end center", bottom_trailing: "end end",
};

/// `Stack`: children share one grid cell, later ones above earlier ones,
/// and the stack is as large as its largest layer.
export function Layers(p, ctx, { align }, children) {
  const node = el("div", { class: "bern-layers" });
  node.style.placeItems = LAYER_PLACE[align ?? "center"];
  children(node, { ...ctx, inGroup: false });
  p.append(ctx.inGroup ? el("div", { class: "bern-row bern-row-media" }, node) : node);
}

/// A picture from a declared resource, served below `resources/`. With
/// one side given, the other follows the picture's aspect ratio. Without
/// a label it is decorative.
export function Image(p, ctx, { source, label, width, height, fit }) {
  const src = `resources/${source.split("/").map(encodeURIComponent).join("/")}`;
  const node = el("img", { class: "bern-image", src, alt: "", draggable: "false", decoding: "async" });
  node.style.objectFit = fit ?? "contain";
  if (width !== undefined) node.style.width = `${width}px`;
  if (height !== undefined) node.style.height = `${height}px`;
  if (label) bindAttr(node, "alt", label);
  p.append(ctx.inGroup ? el("div", { class: "bern-row bern-row-media" }, node) : node);
}

/// A progress bar of `value` out of `total`, or without a value an
/// indeterminate spinner.
export function Progress(p, ctx, { value, total, label }) {
  const caption = label ? el("span", { class: "bern-progress-label" }) : null;
  if (label) bindText(caption, label);
  let indicator;
  if (value) {
    const max = total ?? 1;
    const fill = el("div", { class: "bern-progress-fill" });
    indicator = el("div", { class: "bern-progress-track", role: "progressbar", "aria-valuemin": "0", "aria-valuemax": String(max) }, fill);
    effect(() => {
      const current = Math.min(Math.max(Number(value()) || 0, 0), max);
      fill.style.width = `${(current / max) * 100}%`;
      indicator.setAttribute("aria-valuenow", String(current));
    });
  } else {
    indicator = el("span", { class: "bern-spinner", role: "progressbar" });
  }
  if (label) bindAttr(indicator, "aria-label", label);
  // A progress bar takes the width it gets, as on the native targets.
  const kind = value ? "bern-progress-bar bern-fill-width" : "bern-progress-indeterminate";
  p.append(el("div", { class: `bern-progress ${kind}` }, value ? caption : indicator, value ? indicator : caption));
}

export function Spacer(p) {
  p.append(el("div", { class: "bern-spacer" }));
}

export function Divider(p) {
  p.append(el("hr", { class: "bern-divider" }));
}

export function ScrollView(p, ctx, { axis }, children) {
  // A scroll view takes the room it gets, as on the native targets.
  const node = el("div", { class: `bern-scroll bern-scroll-${axis ?? "vertical"} bern-fill-width bern-fill-height` });
  children(node, ctx);
  p.append(node);
}

export function Form(p, ctx, { title }, children) {
  if (title) pageTitle(ctx, title);
  const node = el("div", { class: "bern-form" });
  children(node, ctx);
  p.append(el("div", { class: "bern-form-scroll" }, node));
}

export function Section(p, ctx, { title, footer }, children) {
  const group = el("div", { class: "bern-group" });
  const node = el("section", { class: "bern-section" });
  if (title) {
    const heading = el("h3", { class: "bern-section-title" });
    bindText(heading, title);
    node.append(heading);
  }
  node.append(group);
  if (footer) {
    const note = el("p", { class: "bern-section-footer" });
    bindText(note, footer);
    node.append(note);
  }
  children(group, { ...ctx, inGroup: true });
  p.append(node);
}

/// Conditional content: `branch()` picks an arm; only a change of arm
/// rebuilds, and the arm's own bindings update in place.
export function When(p, ctx, branch, arms) {
  const region = new Region(p);
  let current = Symbol("none");
  let owner = null;
  let first = true;
  const clock = hooks.clock?.() ?? null;
  effect(() => {
    const next = branch();
    if (next === current) return;
    current = next;
    const animated = !first;
    first = false;
    untrack(() => {
      owner?.dispose();
      region.clear();
      owner = new Scope(null);
      const arm = arms[next];
      if (arm) {
        within(owner, () => structural(animated, () => withClock(clock, () => region.fill((fragment) => arm(fragment, ctx)))));
      }
    });
  });
  onCleanup(() => owner?.dispose());
}

/// The nodes from `start` to `end`, inclusive.
function range(start, end) {
  const nodes = [];
  for (let node = start; node; node = node.nextSibling) {
    nodes.push(node);
    if (node === end) break;
  }
  return nodes;
}

/// Keyed rows: a row keeps its DOM, controls and state while its key
/// stays; its value is a cell the row's bindings read. Each row is the
/// range between two markers, so rows add no wrapper elements.
export function Each(p, ctx, { items, key, stagger }, row) {
  const region = new Region(p);
  let rows = new Map();
  let first = true;
  // Rows added in one update appear one after another with `stagger`;
  // otherwise they follow the stagger of the container they are in.
  const clock = stagger ? hooks.makeClock?.(stagger) ?? null : hooks.clock?.() ?? null;
  // A staggering list that appears in an update plays its rows in turn.
  if (stagger && clock) clock.play = isUpdating();
  effect(() => {
    const list = items();
    const animated = !first;
    first = false;
    untrack(() => structural(animated, () => withClock(clock, () => {
      const next = new Map();
      for (const item of list) {
        const id = key(item);
        let entry = rows.get(id);
        if (entry) {
          entry.value.set(item);
          rows.delete(id);
        } else {
          const value = cell(item);
          const owner = new Scope(null);
          const fragment = document.createDocumentFragment();
          const start = document.createComment("");
          const end = document.createComment("");
          fragment.append(start);
          const rowKey = () => String(key(value.peek()));
          within(owner, () => row(fragment, { ...ctx, key: rowKey, keys: [...(ctx.keys ?? []), rowKey] }, () => value.get()));
          // The row's element, known by its key: scroll views find their
          // items this way.
          const element = [...fragment.childNodes].find((node) => node.nodeType === 1);
          if (element) {
            element.dataset.bernItem = "";
            element.bernKey = key(item);
          }
          fragment.append(end);
          region.end.before(fragment);
          entry = { value, owner, start, end };
        }
        next.set(id, entry);
      }
      for (const stale of rows.values()) {
        stale.owner.dispose();
        for (const node of range(stale.start, stale.end)) leave(node);
      }
      // Reorder in place: move only rows that are out of position.
      let anchor = region.start;
      for (const entry of next.values()) {
        if (anchor.nextSibling !== entry.start) anchor.after(...range(entry.start, entry.end));
        anchor = entry.end;
      }
      rows = next;
    })));
  });
  onCleanup(() => {
    for (const entry of rows.values()) entry.owner.dispose();
  });
}

export function List(p, ctx, children) {
  const node = el("div", { class: "bern-list" });
  children(node, ctx);
  p.append(node);
}

/// A sidebar and detail split. On narrow windows the sidebar is the first
/// page and choosing a destination shows the detail with a back button.
export function SplitView(p, ctx, spec, detail) {
  const { selection, sections, search, id, selectedLabel } = spec;
  const showingDetail = cell(false);
  const list = el("div", { class: "bern-sidebar-list", role: "listbox", tabindex: "0" });
  const sidebar = el("nav", { class: "bern-sidebar" });
  if (search) {
    const holder = el("div", { class: "bern-sidebar-search" });
    search(holder, ctx);
    sidebar.append(holder);
  }
  sidebar.append(list);
  const items = [];
  for (const section of sections) {
    const group = el("div", { class: "bern-sidebar-section", role: "group" });
    if (section.title) {
      const heading = el("div", { class: "bern-sidebar-heading" });
      bindText(heading, section.title);
      group.append(heading);
    }
    for (const item of section.items) {
      const text = el("span");
      bindText(text, item.label);
      const option = el("div", { class: "bern-sidebar-item", role: "option" }, item.icon ? icon(item.icon, 16) : null, text);
      option.addEventListener("click", () => {
        selection.set(item.value);
        showingDetail.set(true);
      });
      items.push({ item, option });
      group.append(option);
    }
    list.append(group);
  }
  effect(() => {
    const value = selection.get();
    for (const { item, option } of items) option.setAttribute("aria-selected", eq(item.value, value) ? "true" : "false");
  });
  list.addEventListener("keydown", (event) => {
    const index = items.findIndex(({ item }) => eq(item.value, selection.peek()));
    const move = { ArrowDown: 1, ArrowUp: -1, Home: -Infinity, End: Infinity }[event.key];
    if (move === undefined) return;
    event.preventDefault();
    const next = Math.min(items.length - 1, Math.max(0, move === -Infinity ? 0 : move === Infinity ? items.length - 1 : index + move));
    selection.set(items[next].item.value);
  });
  const select = (name) => {
    const found = items.find(({ item }) => item.name === name);
    if (!found) return false;
    selection.set(found.item.value);
    showingDetail.set(true);
    return true;
  };
  if (id) {
    mark(list, ctx, id);
    control(ctx, {
      target: () => target(ctx, id), role: "sidebar", label: selectedLabel,
      actions: { select, value: () => items.find(({ item }) => eq(item.value, selection.peek()))?.item.name ?? null },
    });
  }
  const inner = { ...ctx, page: page() };
  const header = pageHeader(inner, selectedLabel, true);
  const back = el("button", { class: "bern-icon-button bern-split-back", type: "button" }, icon("chevron.left", 18));
  bindAttr(back, "aria-label", () => tr("bern.navigation.back"));
  back.addEventListener("click", () => showingDetail.set(false));
  header.prepend(back);
  const body = el("div", { class: "bern-detail-body" });
  const column = el("div", { class: "bern-detail" }, header, body);
  detail(body, inner);
  const node = el("div", { class: "bern-split" }, sidebar, column);
  effect(() => node.classList.toggle("bern-split-detail", showingDetail.get()));
  p.append(node);
}

/// Push navigation over a typed route path; the top route's destination
/// replaces the content and the page header offers back.
export function Navigation(p, ctx, { id, path }, content, destination) {
  const depth = () => path.get().length;
  const back = () => {
    const routes = path.peek();
    if (!routes.length) return false;
    path.set(routes.slice(0, -1));
    return true;
  };
  const inner = { ...ctx, path };
  // The root page stays mounted under pushed destinations, so its
  // subscriptions, players and scroll position survive going back.
  const root = el("div", { class: "bern-nav-page", tabindex: "-1" });
  const top = el("div", { class: "bern-nav-page", tabindex: "-1" });
  top.addEventListener("keydown", (event) => {
    const escape = event.key === "Escape" && !event.isComposing;
    if ((escape || (event.altKey && event.key === "ArrowLeft")) && !event.defaultPrevented && back()) {
      event.preventDefault();
    }
  });
  p.append(root, top);
  content(root, inner);
  const region = new Region(top);
  let shown = 0;
  let owner = null;
  // Focus moves into a pushed page and, going back, returns to what was
  // focused when that page was pushed.
  const returns = [];
  let mounted = false;
  effect(() => {
    const routes = path.get();
    root.hidden = routes.length > 0;
    top.hidden = routes.length === 0;
    if (routes.length === shown && routes.length === 0) {
      mounted = true;
      return;
    }
    const deeper = routes.length > shown;
    const shallower = routes.length < shown;
    if (deeper) returns.push(document.activeElement);
    shown = routes.length;
    if (mounted && (deeper || shallower)) {
      const back = shallower ? returns[routes.length] : null;
      if (shallower) returns.length = routes.length;
      requestAnimationFrame(() => {
        if (back?.isConnected && !back.closest("[hidden]")) back.focus({ preventScroll: true });
        else (routes.length ? top : root).focus({ preventScroll: true });
      });
    }
    untrack(() => {
      owner?.dispose();
      region.clear();
      owner = null;
      if (routes.length === 0) return;
      owner = new Scope(null);
      within(owner, () => {
        if (ctx.page) stackPush(ctx.page.backs, back);
        const route = routes[routes.length - 1];
        region.fill((fragment) => destination(fragment, inner, () => route));
      });
    });
    mounted = true;
  });
  onCleanup(() => owner?.dispose());
  if (id) {
    control(ctx, {
      target: () => target(ctx, id), role: "navigation", label: () => "",
      actions: { back, value: () => String(path.peek().length) },
    });
  }
  return depth;
}

export function NavLink(p, ctx, { id, route, label }, children) {
  const node = el("a", { class: "bern-navlink", href: "#", role: "link" });
  const content = el("span", { class: "bern-navlink-content" });
  children(content, ctx);
  node.append(content, icon("chevron.right", 14));
  const activate = () => {
    if (ctx.path) ctx.path.set([...ctx.path.peek(), route()]);
  };
  node.addEventListener("click", (event) => {
    event.preventDefault();
    activate();
  });
  mark(node, ctx, id);
  if (ctx.path) {
    control(ctx, { target: () => target(ctx, id), role: "link", label, actions: { activate } });
  }
  p.append(node);
}

/// Underline tabs. At a wide dialog's root they become a vertical
/// navigation beside the content.
export function Tabs(p, ctx, { id, selection, tabs, selectedLabel }) {
  const bar = el("div", { class: "bern-tabbar", role: "tablist" });
  const panels = el("div", { class: "bern-tabpanels", role: "tabpanel" });
  const node = el("div", { class: "bern-tabs" }, bar, panels);
  const entries = tabs.map((tab) => {
    const text = el("span");
    bindText(text, tab.label);
    const button = el("button", { class: "bern-tab", type: "button", role: "tab" }, tab.icon ? icon(tab.icon, 16) : null, text);
    button.addEventListener("click", () => selection.set(tab.value));
    button.addEventListener("keydown", (event) => {
      const vertical = node.classList.contains("bern-tabs-vertical");
      const keys = vertical ? { ArrowDown: 1, ArrowUp: -1 } : { ArrowRight: 1, ArrowLeft: -1 };
      const move = { ...keys, Home: -Infinity, End: Infinity }[event.key];
      if (move === undefined) return;
      event.preventDefault();
      const index = tabs.indexOf(tab);
      const at = move === -Infinity ? 0 : move === Infinity ? tabs.length - 1 : (index + move + tabs.length) % tabs.length;
      selection.set(tabs[at].value);
      entries[at].button.focus();
    });
    bar.append(button);
    return { tab, button };
  });
  if (ctx.win?.dialog) {
    const observer = new ResizeObserver(() => {
      const content = node.parentElement;
      const vertical = !!content?.classList.contains("bern-window-content") && content.clientWidth >= 640;
      node.classList.toggle("bern-tabs-vertical", vertical);
      bar.setAttribute("aria-orientation", vertical ? "vertical" : "horizontal");
    });
    observer.observe(node);
    onCleanup(() => observer.disconnect());
  }
  const region = new Region(panels);
  let shown = null;
  let owner = null;
  effect(() => {
    const value = selection.get();
    for (const { tab, button } of entries) {
      const selected = eq(tab.value, value);
      button.setAttribute("aria-selected", selected ? "true" : "false");
      button.tabIndex = selected ? 0 : -1;
    }
    const tab = tabs.find((candidate) => eq(candidate.value, value));
    if (tab === shown) return;
    shown = tab;
    untrack(() => {
      owner?.dispose();
      region.clear();
      owner = new Scope(null);
      if (tab) within(owner, () => region.fill((fragment) => tab.build(fragment, ctx)));
    });
  });
  onCleanup(() => owner?.dispose());
  if (id) {
    mark(bar, ctx, id);
    control(ctx, {
      target: () => target(ctx, id), role: "tabs", label: selectedLabel,
      actions: {
        select: (name) => {
          const tab = tabs.find((candidate) => candidate.name === name);
          if (!tab) return false;
          selection.set(tab.value);
          return true;
        },
        value: () => tabs.find((tab) => eq(tab.value, selection.peek()))?.name ?? null,
      },
    });
  }
  p.append(node);
}

/// Chooses one layout by the available width. Engine state and session
/// drafts live outside the view tree, so switching keeps them; focus moves
/// to the control with the same identity in the new layout.
const SIZES = ["compact", "medium", "expanded", "large"];

/// The size of a width by the breakpoints of `[layouts]`.
export function sizeOf(width, breakpoints) {
  if (width < breakpoints.compact) return "compact";
  if (breakpoints.medium && width < breakpoints.medium) return "medium";
  if (!breakpoints.expanded || width < breakpoints.expanded) return "expanded";
  return "large";
}

/// The layout a size shows: its own, else the next larger declared one,
/// else the next smaller.
function layoutFor(size, layouts) {
  if (layouts[size]) return size;
  const at = SIZES.indexOf(size);
  return [...SIZES.slice(at + 1), ...SIZES.slice(0, at).reverse()].find((name) => layouts[name]);
}

export function Adaptive(p, ctx, { id, breakpoints, layouts }) {
  const node = el("div", { class: "bern-adaptive" });
  const size = cell(null);
  const observer = new ResizeObserver(() => {
    // A hidden container has no width to judge by; keep the last size.
    if (!node.getClientRects().length) return;
    size.set(sizeOf(node.clientWidth, breakpoints));
  });
  observer.observe(node);
  onCleanup(() => observer.disconnect());
  const region = new Region(node);
  let owner = null;
  let shown = null;
  effect(() => {
    const current = layoutFor(size.get() ?? "expanded", layouts);
    if (current === shown) return;
    shown = current;
    untrack(() => {
      const focused = node.contains(document.activeElement)
        ? document.activeElement.closest("[data-bern-target]")?.getAttribute("data-bern-target")
        : null;
      owner?.dispose();
      region.clear();
      owner = new Scope(null);
      within(owner, () => region.fill((fragment) => layouts[current](fragment, ctx)));
      if (focused) {
        // A control inside a closed menu is reached through its button.
        const next = node.querySelector(`[data-bern-target="${CSS.escape(focused)}"]`);
        const closed = next?.closest("[popover]:not(:popover-open)");
        (closed ? node.querySelector(`[popovertarget="${closed.id}"]`) : next)?.focus();
      }
    });
  });
  onCleanup(() => owner?.dispose());
  if (id) {
    control(ctx, {
      target: () => target(ctx, id), role: "adaptive", label: () => "",
      actions: { value: () => size.peek() ?? "expanded" },
    });
  }
  p.append(node);
}

/// A menu button (WAI-ARIA APG Menu Button): the menu is a popover in the
/// top layer and takes focus while open. Each item runs its command through
/// the same closure external entries activate, open or not.
export function Menu(p, ctx, { id, label, icon: name }, items) {
  const menuId = `bern-menu-${++fieldSequence}`;
  const text = el("span");
  bindText(text, label);
  // In a toolbar a menu with an icon shows only the icon.
  const iconOnly = Boolean(name) && ctx.inToolbar;
  const trigger = el("button", {
    class: iconOnly ? "bern-icon-button bern-menu-button" : "bern-button bern-button-standard bern-menu-button", type: "button",
    "aria-haspopup": "menu", "aria-expanded": "false", "aria-controls": menuId, popovertarget: menuId,
  }, name ? icon(name, 16) : null, iconOnly ? null : text, iconOnly ? null : icon("chevron.down", 16));
  if (iconOnly) {
    bindAttr(trigger, "aria-label", label);
    bindAttr(trigger, "title", label);
  }
  const list = el("div", { class: "bern-menu", role: "menu", id: menuId, popover: "auto" });
  bindAttr(list, "aria-label", label);
  const buttons = [];
  for (const item of items) {
    if (!item) {
      list.append(el("div", { class: "bern-menu-separator", role: "separator" }));
      continue;
    }
    const itemText = el("span", { class: "bern-menu-text" });
    bindText(itemText, item.label);
    const button = el("button", {
      class: `bern-menu-item${item.role === "destructive" ? " bern-destructive" : ""}`, type: "button",
      role: "menuitem", tabindex: "-1",
    }, item.icon ? icon(item.icon, 16) : null, itemText);
    const isEnabled = enabledOf(item.enabled);
    effect(() => {
      button.disabled = !isEnabled();
    });
    mark(button, ctx, item.id);
    button.addEventListener("click", () => {
      list.hidePopover();
      trigger.focus();
      item.action();
    });
    button.addEventListener("pointermove", () => {
      if (!button.disabled && document.activeElement !== button) button.focus();
    });
    control(ctx, {
      target: () => target(ctx, item.id), role: "button", label: item.label, enabled: isEnabled,
      actions: { activate: item.action, ...(item.setFile ? { setFile: item.setFile, fileKinds: item.fileKinds } : {}) },
    });
    list.append(button);
    buttons.push(button);
  }
  const enabled = () => buttons.filter((button) => !button.disabled);
  const focusAt = (index) => {
    const available = enabled();
    if (!available.length) return;
    available[(index + available.length) % available.length].focus();
  };
  let focusLast = false;
  list.addEventListener("beforetoggle", (event) => {
    const open = event.newState === "open";
    trigger.setAttribute("aria-expanded", open ? "true" : "false");
    if (open) placePopover(list, trigger);
  });
  list.addEventListener("toggle", (event) => {
    if (event.newState === "open") focusAt(focusLast ? -1 : 0);
    focusLast = false;
  });
  trigger.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      focusLast = event.key === "ArrowUp";
      list.showPopover();
    }
  });
  list.addEventListener("keydown", (event) => {
    const available = enabled();
    const at = available.indexOf(document.activeElement);
    const move = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: available.length - 1 }[event.key];
    if (move !== undefined) {
      event.preventDefault();
      focusAt(move);
    } else if (event.key === "Escape") {
      // Handled here, so an enclosing dialog does not also close.
      event.preventDefault();
      list.hidePopover();
      trigger.focus();
    } else if (event.key === "Tab") {
      list.hidePopover();
    } else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const letter = event.key.toLowerCase();
      const from = Math.max(at, 0);
      for (let step = 1; step <= available.length; step++) {
        const candidate = available[(from + step) % available.length];
        if (candidate.textContent.trim().toLowerCase().startsWith(letter)) {
          candidate.focus();
          break;
        }
      }
    }
  });
  onCleanup(() => {
    if (list.matches(":popover-open")) list.hidePopover();
  });
  mark(trigger, ctx, id);
  control(ctx, {
    target: () => target(ctx, id), role: "menu", label,
    actions: { value: () => (list.matches(":popover-open") ? "open" : "closed") },
  });
  const holder = el("div", { class: "bern-menu-wrap" }, trigger, list);
  p.append(ctx.inGroup && !ctx.inToolbar ? el("div", { class: "bern-row bern-row-button" }, holder) : holder);
}

/// Switches the interface language among the supported languages; each is
/// listed by its own name.
export function LanguagePicker(p, ctx, { id, label }) {
  const options = supportedLanguages.map((tag) => ({ value: tag, name: tag, label: () => autonym(tag) }));
  const field = { get: () => languageCell.get(), peek: () => languageCell.peek(), set: setLanguage };
  const select = (name) => {
    if (!supportedLanguages.includes(name)) return false;
    setLanguage(name);
    return true;
  };
  const node = dropdown(options, field, select);
  mark(node.control, ctx, id);
  control(ctx, {
    target: () => target(ctx, id), role: "picker", label,
    actions: { select, value: () => languageCell.peek() },
  });
  if (ctx.inToolbar) p.append(node);
  else p.append(labelled(node, label, false, node.control));
}

/// The result of an asynchronous query. The subscription belongs to the
/// window instance and ends with this view; parameters are sent again only
/// when they change.
export function Query(p, ctx, spec, template) {
  const { id, query, params, decode, loading, failed, empty, emptyIcon, count, shown } = spec;
  const state = cell({ version: 0n, phase: 0, data: null, error: null });
  const node = el("div", { class: "bern-query" });
  let subscription = null;
  const receive = (bytes) => {
    const reader = new Reader(bytes);
    const version = reader.u64();
    const phase = reader.u8();
    let data = null;
    let error = null;
    if (phase === 2 || phase === 3) data = decode(reader);
    if (phase === 4) {
      if (reader.u8() === 1) data = decode(reader);
      error = { code: reader.text(), message: reader.text() };
    }
    state.set({ version, phase, data, error });
  };
  try {
    subscription = host.queryOpen(ctx.win, query, receive);
  } catch (error) {
    state.set({ version: 0n, phase: 4, data: null, error: { code: error.code ?? "failed", message: error.message } });
  }
  onCleanup(() => {
    if (subscription !== null) host.queryClose(subscription);
  });
  let sent = null;
  const request = (bytes) => {
    if (subscription === null) return;
    try {
      receive(host.queryRequest(subscription, bytes));
    } catch (error) {
      reportError(error);
    }
  };
  effect(() => {
    const bytes = params();
    const key = bytes.join(",");
    if (key === sent) return;
    sent = key;
    untrack(() => request(bytes));
  });
  const retry = () => {
    sent = null;
    const bytes = untrack(params);
    sent = bytes.join(",");
    request(bytes);
  };
  const status = el("div", { class: "bern-query-status" });
  const content = el("div", { class: "bern-query-content" });
  node.append(status, content);
  const retryButton = () => {
    const text = el("span");
    bindText(text, () => tr("bern.query.retry"));
    const button = el("button", { class: "bern-button bern-button-standard", type: "button" }, text);
    button.addEventListener("click", retry);
    return button;
  };
  const spinner = () => el("span", { class: "bern-spinner", "aria-hidden": "true" });
  const phaseKey = () => {
    const { phase, data } = state.get();
    const visible = data === null ? null : shown(data);
    if (phase === 1 || (phase === 0 && data === null)) return "loading";
    if (phase === 4 && data === null) return "failed";
    if ((phase === 2 || phase === 3 || phase === 4) && visible === null && empty) return "empty";
    return "data";
  };
  When(status, ctx, () => {
    const { phase } = state.get();
    const key = phaseKey();
    if (key === "data" && phase === 3) return "refreshing";
    if (key === "data" && phase === 4) return "stale";
    return key === "data" ? "none" : key;
  }, {
    loading: (f) => {
      const text = el("span");
      bindText(text, loading);
      f.append(el("div", { class: "bern-query-center", role: "status" }, spinner(), text));
    },
    failed: (f) => {
      const text = el("p", { class: "bern-query-message" });
      bindText(text, failed);
      f.append(el("div", { class: "bern-query-center", role: "alert" }, icon("exclamationmark.triangle", 32), text, retryButton()));
    },
    empty: (f) => {
      const text = el("p", { class: "bern-query-message" });
      bindText(text, empty);
      f.append(el("div", { class: "bern-query-center" }, icon(emptyIcon ?? "tray", 32), text));
    },
    refreshing: (f) => {
      const text = el("span");
      bindText(text, loading);
      f.append(el("div", { class: "bern-query-line", role: "status" }, spinner(), text));
    },
    stale: (f) => {
      const text = el("span");
      bindText(text, failed);
      f.append(el("div", { class: "bern-query-line bern-query-error", role: "alert" }, icon("exclamationmark.triangle", 14), text, retryButton()));
    },
  });
  When(content, ctx, () => (phaseKey() === "data" ? "data" : "none"), {
    data: (f) => template(f, ctx, () => {
      const { data } = state.get();
      return data === null ? null : shown(data);
    }),
  });
  if (id) {
    control(ctx, {
      target: () => target(ctx, id), role: "query", label: () => "",
      actions: {
        activate: retry,
        value: () => {
          const { phase, version, data } = state.peek();
          const name = ["idle", "loading", "ready", "refreshing", "failed"][phase];
          const n = data === null ? null : count(data);
          return n === null ? `${name} v${version}` : `${name} v${version} (${n})`;
        },
      },
    });
  }
  p.append(node);
}

// MARK: - Media

const MEDIA = { players: 0, state: 1, open: 2, play: 3, pause: 4, seek: 5, set_volume: 6, set_muted: 7 };
const PHASES = ["Idle", "Loading", "Ready", "Playing", "Paused", "Buffering", "Ended", "Failed", "Unavailable"];
const PHASE = Object.fromEntries(PHASES.map((name, index) => [name, index]));

/// One `MediaPlayer` record of the media contract, in its field order.
function readPlayer(r) {
  const state = {
    id: r.text(), owner: r.text(), sourceBound: r.bool(), source: readOptional(r, (r) => r.text()),
    sourceVersion: r.i64(), phase: r.u32(), positionMs: r.i64(), durationMs: readOptional(r, (r) => r.i64()),
    seekable: r.bool(), volume: r.f64(), muted: r.bool(), error: readOptional(r, (r) => r.text()), views: r.i64(),
  };
  // The latest request is only shown to external entries.
  readOptional(r, (r) => [r.text(), r.u32(), r.i64(), readOptional(r, (r) => r.text())]);
  return state;
}

/// Runs the hub's requests with one HTML video element per player and
/// reports what the element does as observations, never as commands.
class HtmlPlayer {
  constructor(id) {
    this.id = id;
    this.version = 0n;
    this.ended = false;
    this.started = false;
    this.waiting = false;
    const video = document.createElement("video");
    video.className = "bern-video-element";
    video.playsInline = true;
    video.preload = "auto";
    // Native controls change playback before scripts hear of it; the Bern
    // control bar sends commands instead.
    video.controls = false;
    video.disablePictureInPicture = true;
    video.setAttribute("aria-hidden", "true");
    this.video = video;
    const report = () => this.report();
    for (const name of ["loadedmetadata", "durationchange", "canplay", "playing", "pause", "seeked", "volumechange", "emptied"]) {
      video.addEventListener(name, report);
    }
    video.addEventListener("waiting", () => {
      this.waiting = true;
      this.report();
    });
    video.addEventListener("playing", () => {
      this.waiting = false;
      this.started = true;
      this.report();
    });
    video.addEventListener("ended", () => {
      this.ended = true;
      this.report();
    });
    video.addEventListener("error", report);
    let last = 0;
    video.addEventListener("timeupdate", () => {
      const now = performance.now();
      if (now - last >= 500) {
        last = now;
        this.report();
      }
    });
  }

  load(url, version) {
    this.version = version;
    this.reset();
    this.video.pause();
    this.video.src = url;
    this.video.load();
    this.report();
  }

  unload(version) {
    this.version = version;
    this.reset();
    this.video.pause();
    this.video.removeAttribute("src");
    this.video.load();
    this.report();
  }

  reset() {
    this.ended = false;
    this.started = false;
    this.waiting = false;
  }

  /// `play()` settles once playback really starts or is refused.
  play() {
    if (this.ended) {
      this.ended = false;
      this.video.currentTime = 0;
    }
    this.started = true;
    return this.video.play();
  }

  release() {
    this.video.pause();
    this.video.removeAttribute("src");
    this.video.load();
    this.video.remove();
  }

  report() {
    const video = this.video;
    let phase;
    let error = null;
    if (!video.getAttribute("src")) phase = PHASE.Idle;
    else if (video.error) {
      phase = PHASE.Failed;
      error = video.error.message || `media error ${video.error.code}`;
    } else if (video.readyState < 1) phase = PHASE.Loading;
    else if (this.ended) phase = PHASE.Ended;
    else if (!video.paused) phase = this.waiting || video.readyState < 3 ? PHASE.Buffering : PHASE.Playing;
    else phase = this.started ? PHASE.Paused : PHASE.Ready;
    const duration = Number.isFinite(video.duration) ? Math.round(video.duration * 1000) : null;
    const writer = new Writer();
    writer.text(this.id);
    writer.u64(this.version);
    writer.u32(phase);
    writer.i64(BigInt(Math.max(0, Math.round((video.currentTime || 0) * 1000))));
    writeOptional(writer, duration === null ? null : BigInt(duration), (w, v) => w.i64(v));
    writer.u8(duration !== null && video.seekable.length > 0 ? 1 : 0);
    writer.f64(Math.round(video.volume * 1000) / 1000);
    writer.u8(video.muted ? 1 : 0);
    writeOptional(writer, error, (w, v) => w.text(v));
    host.mediaPost("bern_media_event", writer.finish());
  }
}

/// Player identities, merged states and the commands of the media hub.
const media = {
  players: cell(new Map()),
  revision: 0n,
  backend: new Map(),

  attach() {
    const writer = new Writer();
    writer.text("html-media-element");
    writer.text("1");
    writer.u8(1); // keeps playing without a view
    writer.u8(0); // one view per player
    host.mediaCall("bern_media_executor_attached", writer.finish());
    this.receive(host.buffer("bern_media_snapshot", host.app));
  },

  receive(bytes) {
    const reader = new Reader(bytes);
    const revision = reader.u64();
    if (revision < this.revision) return;
    this.revision = revision;
    const players = new Map();
    const count = reader.u32();
    for (let i = 0; i < count; i++) {
      const player = readPlayer(reader);
      players.set(player.id, player);
    }
    this.players.set(players);
  },

  player(id) {
    let player = this.backend.get(id);
    if (!player) {
      player = new HtmlPlayer(id);
      this.backend.set(id, player);
    }
    return player;
  },

  /// Perform one hub request: request, player, source version, operation.
  request(bytes) {
    const reader = new Reader(bytes);
    const request = reader.u64();
    const id = reader.text();
    const version = reader.u64();
    const operation = reader.u32();
    const complete = (failure) => {
      const writer = new Writer();
      writer.u64(request);
      if (failure) {
        writer.u8(1);
        writer.text(failure.code);
        writer.text(failure.message);
      } else writer.u8(0);
      host.mediaPost("bern_media_complete", writer.finish());
    };
    try {
      if (operation === 0) {
        const url = reader.text();
        this.player(id).load(new URL(url, document.baseURI).href, version);
        return complete(null);
      }
      if (operation === 1) {
        this.player(id).unload(version);
        return complete(null);
      }
      if (operation === 7) {
        this.backend.get(id)?.release();
        this.backend.delete(id);
        return complete(null);
      }
      const player = this.player(id);
      if (player.version !== version) {
        return complete(new CommandFailure("stale_source", "a newer source is loaded"));
      }
      switch (operation) {
        case 2:
          player.play().then(
            () => complete(null),
            (error) => {
              player.started = false;
              player.report();
              complete(error?.name === "NotAllowedError"
                ? new CommandFailure("user_activation_required", "the browser allows playback only after a user gesture on the page")
                : new CommandFailure("media_failed", String(error?.message ?? error)));
            },
          );
          return;
        case 3:
          player.video.pause();
          return complete(null);
        case 4: {
          const position = Number(reader.i64()) / 1000;
          player.ended = false;
          player.video.addEventListener("seeked", () => complete(null), { once: true });
          player.video.currentTime = position;
          return;
        }
        case 5:
          player.video.volume = Math.min(1, Math.max(0, reader.f64()));
          return complete(null);
        case 6:
          player.video.muted = reader.u8() !== 0;
          return complete(null);
        default:
          return complete(new CommandFailure("unknown_command", `unknown media operation ${operation}`));
      }
    } catch (error) {
      complete(new CommandFailure("invalid_request", String(error?.message ?? error)));
    }
  },

  command(ctx, command, player, encode = () => {}) {
    host.dispatchMedia(MEDIA[command], (writer) => {
      writer.text(player);
      encode(writer);
    }, ctx?.win ?? null);
  },

  create(player, owner) {
    const writer = new Writer();
    writer.text(player);
    writer.text(owner);
    return host.status(host.mediaCall("bern_media_create", writer.finish()));
  },

  bind(player, url) {
    const writer = new Writer();
    writer.text(player);
    writeOptional(writer, url, (w, v) => w.text(v));
    return host.status(host.mediaCall("bern_media_bind", writer.finish()));
  },

  release(player) {
    const writer = new Writer();
    writer.text(player);
    host.mediaPost("bern_media_release", writer.finish());
  },

  view(token, player, window) {
    const writer = new Writer();
    writer.text(token);
    writer.u8(player ? 1 : 0);
    if (player) {
      writer.text(player);
      writer.text(window);
    }
    host.mediaPost("bern_media_view", writer.finish());
  },
};

export function mediaOpen(ctx, player, url) {
  media.command(ctx, "open", player, (writer) => writer.text(url));
}

export function mediaCommand(ctx, command, player, value) {
  media.command(ctx, command, player, (writer) => {
    if (command === "seek") writer.i64(value);
    if (command === "set_volume") writer.f64(value);
    if (command === "set_muted") writer.u8(value ? 1 : 0);
  });
}

function time(milliseconds) {
  const seconds = Math.floor(Math.max(Number(milliseconds), 0) / 1000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = String(seconds % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
}

function describe(state) {
  const duration = state.durationMs === null ? "--:--" : time(state.durationMs);
  return `${PHASES[state.phase]} ${time(state.positionMs)} / ${duration}`;
}

let viewSequence = 0;

/// A video of a declaration's own player (`source`) or of an application
/// player, with the standard command-driven control bar.
export function Video(p, ctx, { id, source, player, label, controls, fit, corner, animate }) {
  const name = label ?? (() => tr("bern.media.video"));
  const playerID = player ?? `${sessionPrefix()}${ctx.win.id}/${target(ctx, id)}`;
  const state = () => media.players.get().get(playerID) ?? null;
  if (source) {
    const failure = media.create(playerID, ctx.win.id);
    if (failure) ctx.win.fail(failure);
    else {
      onCleanup(() => media.release(playerID));
      let bound;
      effect(() => {
        const url = source();
        if (url === bound) return;
        bound = url;
        const failed = untrack(() => media.bind(playerID, url));
        if (failed) ctx.win.fail(failed);
      });
    }
  }
  const surface = el("div", { class: `bern-video-surface bern-video-${fit ?? "contain"}` });
  bindAttr(surface, "aria-label", name);
  surface.setAttribute("role", "img");
  const status = el("div", { class: "bern-video-status" });
  surface.append(status);
  // The player's element shows here while this view is attached.
  const token = `view-${++viewSequence}`;
  const element = media.player(playerID).video;
  surface.prepend(element);
  media.view(token, playerID, ctx.win.id);
  onCleanup(() => {
    media.view(token, null, null);
    if (element.parentNode === surface) element.remove();
  });
  const shows = (s) => s && [PHASE.Ready, PHASE.Playing, PHASE.Paused, PHASE.Buffering, PHASE.Ended].includes(s.phase);
  effect(() => {
    const s = state();
    element.style.visibility = shows(s) ? "visible" : "hidden";
    status.replaceChildren();
    if (!s || s.phase === PHASE.Unavailable) {
      status.append(icon("play.rectangle", 32), el("span", {}, tr("bern.media.unavailable")));
    } else if (s.phase === PHASE.Idle) {
      status.append(icon("play.rectangle", 32), el("span", {}, tr("bern.media.empty")));
    } else if (s.phase === PHASE.Loading) {
      status.append(el("span", { class: "bern-spinner", role: "status" }));
    } else if (s.phase === PHASE.Failed) {
      status.append(icon("exclamationmark.triangle", 32), el("strong", {}, tr("bern.media.failed")), el("span", {}, s.error ?? ""));
    }
    status.hidden = status.childElementCount === 0;
    surface.setAttribute("aria-description", s ? describe(s) : "");
  });
  control(ctx, {
    target: () => target(ctx, id), role: "video", label: name,
    actions: { value: () => {
      const s = media.players.peek().get(playerID);
      return s ? describe(s) : null;
    } },
  });
  const node = el("div", { class: "bern-video bern-fill-width" }, surface);
  if (controls !== false) node.append(videoControls(ctx, id, playerID, state));
  // A rounded picture: the surface clips the video to its corners.
  if (corner !== undefined) {
    surface.classList.add("bern-video-rounded");
    hooks.length?.(surface, "borderRadius", corner, animate);
  }
  p.append(node);
}

/// The media progress bar, a control of its own rather than the Slider:
/// neutral progress and a capsule knob. Pointer drags and keys update a
/// preview; releasing the pointer or the key commits once.
function scrubber({ label, text, step, page, preview, commit }) {
  const fill = el("div", { class: "bern-scrubber-fill" });
  const knob = el("div", { class: "bern-scrubber-knob" });
  const node = el("div", { class: "bern-scrubber", role: "slider", tabindex: "0", "aria-valuemin": "0" },
    el("div", { class: "bern-scrubber-track" }, fill), knob);
  bindAttr(node, "aria-label", label);
  let max = 1;
  let value = 0;
  let pending = null;
  let disabled = false;
  const clamp = (number) => Math.min(max, Math.max(0, number));
  const render = () => {
    const shown = pending ?? value;
    node.style.setProperty("--bern-progress", String(max > 0 ? clamp(shown) / max : 0));
    node.setAttribute("aria-valuemax", String(max));
    node.setAttribute("aria-valuenow", String(Math.round(shown * 1000) / 1000));
    node.setAttribute("aria-valuetext", text(shown));
  };
  const move = (next) => {
    pending = clamp(next);
    preview?.(pending);
    render();
  };
  const finish = () => {
    if (pending === null) return;
    const next = pending;
    pending = null;
    commit(next);
    render();
  };
  const at = (event) => {
    const rect = node.getBoundingClientRect();
    const half = knob.offsetWidth / 2;
    return ((event.clientX - rect.left - half) / Math.max(1, rect.width - 2 * half)) * max;
  };
  node.addEventListener("pointerdown", (event) => {
    if (disabled || event.button !== 0) return;
    event.preventDefault();
    node.focus();
    node.setPointerCapture(event.pointerId);
    node.classList.add("bern-scrubber-dragging");
    move(at(event));
  });
  node.addEventListener("pointermove", (event) => {
    if (node.hasPointerCapture(event.pointerId)) move(at(event));
  });
  const release = (event) => {
    if (!node.hasPointerCapture(event.pointerId)) return;
    node.releasePointerCapture(event.pointerId);
    node.classList.remove("bern-scrubber-dragging");
    finish();
  };
  node.addEventListener("pointerup", release);
  node.addEventListener("pointercancel", release);
  const KEYS = ["ArrowLeft", "ArrowDown", "ArrowRight", "ArrowUp", "PageDown", "PageUp", "Home", "End"];
  node.addEventListener("keydown", (event) => {
    if (disabled || !KEYS.includes(event.key)) return;
    event.preventDefault();
    const from = pending ?? value;
    const by = { ArrowLeft: -step, ArrowDown: -step, ArrowRight: step, ArrowUp: step, PageDown: -page(max), PageUp: page(max) };
    move(event.key === "Home" ? 0 : event.key === "End" ? max : from + by[event.key]);
  });
  node.addEventListener("keyup", (event) => {
    if (KEYS.includes(event.key)) finish();
  });
  node.addEventListener("blur", finish);
  return {
    node,
    previewing: () => pending !== null,
    update(next) {
      max = Math.max(next.max, 0);
      if (pending === null) value = next.value;
      disabled = next.disabled;
      node.setAttribute("aria-disabled", disabled ? "true" : "false");
      node.tabIndex = disabled ? -1 : 0;
      render();
    },
  };
}

function videoControls(ctx, id, playerID, state) {
  const current = () => media.players.peek().get(playerID) ?? null;
  const playing = (s) => s && (s.phase === PHASE.Playing || s.phase === PHASE.Buffering);
  const playable = (s) => s && s.source !== null && ![PHASE.Idle, PHASE.Failed, PHASE.Unavailable].includes(s.phase);
  const seekable = (s) => playable(s) && s.seekable && s.durationMs !== null;
  const controllable = (s) => s && s.phase !== PHASE.Unavailable;

  const togglePlayback = () => media.command(ctx, playing(current()) ? "pause" : "play", playerID);
  const seek = (ms) => media.command(ctx, "seek", playerID, (w) => w.i64(BigInt(Math.round(ms))));
  const setMuted = (muted) => media.command(ctx, "set_muted", playerID, (w) => w.u8(muted ? 1 : 0));
  const setVolume = (volume) => media.command(ctx, "set_volume", playerID, (w) => w.f64(Math.min(1, Math.max(0, volume))));

  const play = el("button", { class: "bern-icon-button", type: "button" });
  play.addEventListener("click", togglePlayback);
  const positionText = el("span", { class: "bern-video-time" });
  const durationText = el("span", { class: "bern-video-time" });
  const position = scrubber({
    label: () => tr("bern.media.position"), text: time, step: 5000, page: (max) => max / 10,
    preview: (ms) => {
      positionText.textContent = time(ms);
    },
    commit: seek,
  });
  position.node.classList.add("bern-video-position");
  const mute = el("button", { class: "bern-icon-button", type: "button" });
  mute.addEventListener("click", () => setMuted(!(current()?.muted ?? false)));
  const volume = scrubber({
    label: () => tr("bern.media.volume"), text: (level) => `${Math.round(level * 100)}%`, step: 0.05,
    page: () => 0.1, commit: setVolume,
  });
  volume.node.classList.add("bern-video-volume");
  effect(() => {
    const s = state();
    const isPlaying = playing(s);
    const playLabel = tr(isPlaying ? "bern.media.pause" : "bern.media.play");
    play.replaceChildren(icon(isPlaying ? "pause.fill" : "play.fill", 16));
    play.setAttribute("aria-label", playLabel);
    play.title = playLabel;
    play.disabled = !playable(s);
    const duration = s?.durationMs === null || !s ? 0 : Number(s.durationMs);
    const at = s ? Number(s.positionMs) : 0;
    position.update({ value: Math.min(at, duration), max: duration, disabled: !seekable(s) });
    if (!position.previewing()) positionText.textContent = time(at);
    durationText.textContent = s?.durationMs != null ? time(s.durationMs) : playable(s) ? tr("bern.media.live") : "--:--";
    const muted = s?.muted ?? false;
    const muteLabel = tr(muted ? "bern.media.unmute" : "bern.media.mute");
    mute.replaceChildren(icon(muted ? "speaker.slash.fill" : "speaker.wave.3.fill", 16));
    mute.setAttribute("aria-label", muteLabel);
    mute.title = muteLabel;
    mute.disabled = !controllable(s);
    volume.update({ value: s?.volume ?? 1, max: 1, disabled: !controllable(s) });
  });
  const register = (name, entry) => control(ctx, { ...entry, target: () => `${target(ctx, id)}.${name}` });
  register("play_pause", {
    role: "button", label: () => tr(playing(state()) ? "bern.media.pause" : "bern.media.play"),
    enabled: () => playable(state()), actions: { activate: togglePlayback },
  });
  register("position", {
    role: "slider", label: () => tr("bern.media.position"), enabled: () => seekable(state()),
    actions: { setNumber: seek, value: () => num(Number(current()?.positionMs ?? 0)) },
  });
  register("mute", {
    role: "switch", label: () => tr("bern.media.mute"), enabled: () => controllable(state()),
    actions: {
      activate: () => setMuted(!(current()?.muted ?? false)), setBool: setMuted,
      value: () => ((current()?.muted ?? false) ? "true" : "false"),
    },
  });
  register("volume", {
    role: "slider", label: () => tr("bern.media.volume"), enabled: () => controllable(state()),
    actions: { setNumber: setVolume, value: () => num(current()?.volume ?? 1) },
  });
  return el("div", { class: "bern-video-controls" }, play, positionText, position.node, durationText, mute, volume.node);
}

// MARK: - Windows

const VISIBLE = 0;
const MINIMIZED = 1;
const HIDDEN = 2;
const ACTION = { activate: 0, close: 1, minimize: 2, restore: 3, maximize: 4, move: 5, resize: 6 };

/// One window instance: its kind, generation, sessions and frame. The main
/// window is the page, other windows are dialogs, and only a declared
/// `presentation: floating` gets a movable frame.
class Win {
  constructor(manager, index, kind) {
    this.manager = manager;
    this.kind = index;
    this.spec = kind;
    this.generation = 0n;
    this.id = "";
    this.sessions = {};
    this.state = cell({ visibility: VISIBLE, maximized: false, frame: null });
    this.pendingClose = cell(null);
    this.failure = cell(null);
    this.commandInFlight = false;
    this.allowsClose = false;
    /// `window.idle`: no input for the window's `idle_after` seconds.
    this.idle = cell(false);
    /// `window.compact` and its siblings: the size of the window's width.
    this.compact = cell(false);
    this.medium = cell(false);
    this.expanded = cell(true);
    this.large = cell(false);
  }

  get floating() {
    return this.spec.presentation === "floating";
  }

  get dialog() {
    return this.spec.presentation === "dialog";
  }

  fail(failure) {
    this.failure.set(failure);
  }

  /// Write one session field through Rust, which validates it.
  set(session, field, encode, value) {
    const writer = new Writer();
    encode(writer, value);
    return host.sessionSet(this, session, field, writer.finish());
  }

  /// Window actions available now, as bits in `WindowAction` order. The
  /// page and dialogs only activate and close.
  actions() {
    const always = (1 << ACTION.activate) | (1 << ACTION.close);
    if (!this.floating) return always;
    const { visibility, maximized } = this.state.peek();
    const narrow = this.manager.narrow.peek();
    const resizable = this.spec.resizable;
    const normal = visibility !== MINIMIZED;
    const bits = [
      [ACTION.minimize, normal],
      [ACTION.restore, visibility === MINIMIZED || (maximized && resizable)],
      [ACTION.maximize, normal && resizable && !maximized],
      [ACTION.move, normal && !maximized && !narrow],
      [ACTION.resize, normal && resizable && !maximized && !narrow],
    ];
    return bits.reduce((all, [bit, on]) => (on ? all | (1 << bit) : all), always);
  }

  /// The window's frame in the page area, from layout: an opening
  /// animation's scale does not change it.
  frame() {
    const desktop = this.manager.desktop.getBoundingClientRect();
    const rect = this.element.getBoundingClientRect();
    const width = this.element.offsetWidth;
    const height = this.element.offsetHeight;
    return {
      x: rect.left + (rect.width - width) / 2 - desktop.left,
      y: rect.top + (rect.height - height) / 2 - desktop.top,
      width,
      height,
    };
  }

  reportFacts() {
    if (!this.generation || !this.element) return;
    const { visibility, maximized } = this.state.peek();
    host.windowFacts(this, {
      visibility: document.hidden ? HIDDEN : visibility,
      size: maximized && visibility !== MINIMIZED ? 1 : 0,
      ...this.frame(),
      focused: this.manager.active.peek() === this && document.hasFocus() && visibility !== MINIMIZED,
      actions: this.actions(),
    });
  }
}

const FOCUSABLE = "button, [href], input, select, textarea, [tabindex]:not([tabindex=\"-1\"])";

function focusables(container) {
  return [...container.querySelectorAll(FOCUSABLE)].filter((node) => !node.disabled && node.getClientRects().length > 0);
}

/// Keep Tab and Shift+Tab inside a modal container.
function trapFocus(container, event) {
  const items = focusables(container);
  if (!items.length) {
    event.preventDefault();
    return;
  }
  const first = items[0];
  const last = items[items.length - 1];
  if (!container.contains(document.activeElement)) {
    event.preventDefault();
    first.focus();
  } else if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

class WindowManager {
  constructor(app, root) {
    this.app = app;
    this.primary = app.primary;
    this.windows = new Map();
    this.order = cell([]);
    this.active = cell(null);
    this.narrow = cell(false);
    this.root = root;
    this.desktop = el("main", { class: "bern-desktop" });
    const name = el("span", { class: "bern-appname" });
    bindText(name, () => tr("app.name"));
    const bar = el("header", { class: "bern-appbar" }, name);
    // The window switcher exists only for floating windows, the only ones
    // that can be minimized or hidden behind each other.
    if (app.windows.some((kind) => kind.presentation === "floating")) {
      this.switcher = el("div", { class: "bern-switcher", role: "toolbar" });
      bindAttr(this.switcher, "aria-label", () => tr("bern.window.switcher"));
      bar.append(this.switcher);
      effect(() => this.renderSwitcher());
    }
    const trailing = el("div", { class: "bern-appbar-trailing" });
    const settings = app.windows.find((kind) => kind.settings);
    if (settings) {
      const button = el("button", { class: "bern-icon-button", type: "button" }, icon("gearshape", 16));
      bindAttr(button, "aria-label", () => tr("bern.window.settings"));
      bindAttr(button, "title", () => tr("bern.window.settings"));
      button.addEventListener("click", () => this.open(settings.id));
      trailing.append(button);
      document.addEventListener("keydown", (event) => {
        if ((event.metaKey || event.ctrlKey) && event.key === ",") {
          event.preventDefault();
          this.open(settings.id);
        }
      });
    }
    bar.append(trailing);
    root.append(bar, this.desktop);
    // Closing the main window by a command leaves the page with a way back.
    const main = app.windows.find((kind) => kind.presentation === "page");
    if (main) {
      const title = el("h1", { class: "bern-empty-title" });
      bindText(title, () => tr("app.name"));
      const reopen = el("button", { class: "bern-button bern-button-prominent", type: "button" });
      bindText(reopen, () => fmt("bern.window.reopen", { title: tr(main.title) }));
      reopen.addEventListener("click", () => this.open(main.id));
      const empty = el("div", { class: "bern-empty-page" }, title, reopen);
      this.desktop.append(empty);
      effect(() => {
        empty.hidden = this.order.get().some((win) => win.spec === main);
      });
    }
    document.addEventListener("keydown", (event) => this.keydown(event));
    const measure = () => this.narrow.set(this.desktop.clientWidth < 768);
    new ResizeObserver(() => {
      measure();
      for (const win of this.windows.values()) win.reportFacts();
    }).observe(this.desktop);
    measure();
    effect(() => root.classList.toggle("bern-narrow", this.narrow.get()));
    const refresh = () => {
      for (const win of this.windows.values()) win.reportFacts();
    };
    document.addEventListener("visibilitychange", refresh);
    window.addEventListener("focus", refresh);
    window.addEventListener("blur", refresh);
  }

  renderSwitcher() {
    const active = this.active.get();
    this.order.get();
    this.switcher.replaceChildren();
    for (const kind of this.app.windows) {
      if (kind.presentation !== "floating") continue;
      const win = this.windows.get(kind.id);
      const state = win?.state.get();
      const text = el("span");
      bindText(text, () => tr(kind.title));
      const button = el("button", { class: "bern-switch-item", type: "button" }, text);
      button.setAttribute("aria-pressed", win && win === active && state.visibility !== MINIMIZED ? "true" : "false");
      if (!win) button.classList.add("bern-switch-closed");
      if (state?.visibility === MINIMIZED) button.classList.add("bern-switch-minimized");
      button.addEventListener("click", () => this.open(kind.id));
      this.switcher.append(button);
    }
  }

  /// The topmost open dialog, which holds keyboard focus.
  topDialog() {
    const dialogs = [...this.windows.values()].filter((win) => win.dialog);
    return dialogs.sort((a, b) => Number(a.root.style.zIndex) - Number(b.root.style.zIndex)).pop() ?? null;
  }

  /// Escape closes the top dialog through the close protocol; Tab stays
  /// inside it. Its own confirmation or alert handles keys first.
  keydown(event) {
    const top = this.topDialog();
    if (!top || event.defaultPrevented) return;
    if (top.pendingClose.peek() || top.failure.peek()) return;
    if (event.key === "Escape" && !event.isComposing) {
      event.preventDefault();
      this.close(top);
    } else if (event.key === "Tab") {
      trapFocus(top.element, event);
    }
  }

  /// Open a window kind, or bring its instance forward.
  open(id) {
    const existing = this.windows.get(id);
    if (existing) {
      if (existing.state.peek().visibility === MINIMIZED) this.setState(existing, { visibility: VISIBLE });
      this.activate(existing);
      return existing;
    }
    const index = this.app.windows.findIndex((kind) => kind.id === id);
    if (index < 0) return null;
    const kind = this.app.windows[index];
    const win = new Win(this, index, kind);
    const reader = new Reader(host.windowOpened(index));
    if (reader.u8() !== 0) {
      reportError(`${reader.text()}: ${reader.text()}`);
      return null;
    }
    win.generation = reader.u64();
    win.id = reader.text();
    const count = reader.u32();
    for (let i = 0; i < count; i++) {
      const sessionIndex = reader.u32();
      const type = this.app.sessions[sessionIndex];
      win.sessions[type.name] = new SessionState(win, sessionIndex, type, reader);
    }
    win.returnFocus = document.activeElement;
    this.windows.set(id, win);
    this.mount(win);
    this.order.set([...this.order.peek(), win]);
    this.activate(win);
    if (win.dialog) {
      // Focus moves into the dialog: its first control, else the dialog.
      requestAnimationFrame(() => {
        const content = win.element.querySelector(".bern-window-content");
        (focusables(content)[0] ?? win.element).focus();
      });
    }
    return win;
  }

  mount(win) {
    const kind = win.spec;
    win.scope = new Scope(null);
    const ctx = { win, s: win.sessions, comp: kind.id, key: null, page: page(), path: null };
    const body = el("div", { class: "bern-window-body" });
    const content = el("div", { class: "bern-window-content" });
    within(win.scope, () => {
      if (win.dialog) this.mountDialog(win, ctx, body);
      else if (win.floating) this.mountFloating(win, body);
      else {
        win.element = el("div", { class: "bern-window bern-window-page", "data-bern-window": kind.id }, body);
        win.root = win.element;
      }
      if (!win.dialog) body.append(pageHeader(ctx, null, false));
      body.append(content);
      if (this.app.layouts) this.sizeWindow(win);
      kind.render(content, ctx);
      if (kind.idleAfter) hooks.idle?.(win, kind.idleAfter);
      // Immersive: the page header and its toolbar hide, and the pointer
      // once the window is idle.
      if (kind.immersive) effect(() => win.element.classList.toggle("bern-immersive", kind.immersive(ctx) === true));
      if (kind.closeCheck) win.closeCheck = kind.closeCheck(win);
      if (win.floating) effect(() => this.place(win));
      effect(() => this.dialog(win));
      effect(() => this.alert(win));
    });
    win.element.addEventListener("pointerdown", () => this.activate(win), true);
    this.desktop.append(win.root);
    requestAnimationFrame(() => win.reportFacts());
  }

  /// Keeps `window.compact` and its siblings to the width of the window.
  sizeWindow(win) {
    const update = () => {
      const width = win.element.clientWidth;
      if (!width) return;
      const size = sizeOf(width, this.app.layouts);
      batch(() => {
        for (const name of SIZES) win[name].set(name === size);
      });
    };
    const observer = new ResizeObserver(update);
    observer.observe(win.element);
    onCleanup(() => observer.disconnect());
    update();
  }

  /// A modal dialog over a scrim: the title and toolbar in its header with
  /// a close button, and no minimize or maximize.
  mountDialog(win, ctx, body) {
    const kind = win.spec;
    const header = pageHeader(ctx, () => tr(kind.title), true);
    header.classList.add("bern-dialog-header");
    header.querySelector(".bern-page-title").id = `bern-title-${kind.id}`;
    const close = el("button", { class: "bern-icon-button", type: "button" }, icon("xmark", 16));
    bindAttr(close, "aria-label", () => tr("bern.window.close"));
    bindAttr(close, "title", () => tr("bern.window.close"));
    close.addEventListener("click", () => this.close(win));
    header.append(close);
    const element = el("section", {
      class: "bern-window bern-window-dialog", role: "dialog", "aria-modal": "true",
      "aria-labelledby": `bern-title-${kind.id}`, "data-bern-window": kind.id, tabindex: "-1",
    }, header, body);
    element.style.setProperty("--bern-dialog-width", `${kind.width ?? 560}px`);
    if (kind.height) element.style.height = `${kind.height}px`;
    const layer = el("div", { class: "bern-dialog-layer" }, element);
    // A click that starts and ends on the scrim requests closing.
    let fromScrim = false;
    layer.addEventListener("pointerdown", (event) => {
      fromScrim = event.target === layer;
    });
    layer.addEventListener("click", (event) => {
      if (fromScrim && event.target === layer) this.close(win);
      fromScrim = false;
    });
    win.element = element;
    win.root = layer;
  }

  /// A floating window: a movable, resizable panel with its own title bar.
  mountFloating(win, body) {
    const kind = win.spec;
    const title = el("h1", { class: "bern-window-title", id: `bern-title-${kind.id}` });
    bindText(title, () => tr(kind.title));
    const button = (name, label, action) => {
      const node = el("button", { class: "bern-icon-button bern-window-control", type: "button" }, icon(name, 16));
      bindAttr(node, "aria-label", label);
      bindAttr(node, "title", label);
      node.addEventListener("click", (event) => {
        event.stopPropagation();
        action();
      });
      return node;
    };
    const controlsBox = el("div", { class: "bern-window-controls" });
    controlsBox.append(button("minus", () => tr("bern.window.minimize"), () => this.command(win, "minimize")));
    if (kind.resizable) {
      const toggle = button("square", () => tr(win.state.get().maximized ? "bern.window.restore" : "bern.window.maximize"),
        () => this.command(win, win.state.peek().maximized ? "restore" : "maximize"));
      effect(() => toggle.replaceChildren(icon(win.state.get().maximized ? "square.on.square" : "square", 16)));
      controlsBox.append(toggle);
    }
    controlsBox.append(button("xmark", () => tr("bern.window.close"), () => this.close(win)));
    const titlebar = el("div", { class: "bern-titlebar" }, title, controlsBox);
    const element = el("section", {
      class: "bern-window bern-window-floating", role: "dialog", "aria-labelledby": `bern-title-${kind.id}`,
      "data-bern-window": kind.id,
    }, titlebar, body);
    win.element = element;
    win.root = element;
    titlebar.addEventListener("dblclick", () => {
      if (kind.resizable) this.command(win, win.state.peek().maximized ? "restore" : "maximize");
    });
    this.draggable(win, titlebar);
    if (kind.resizable) this.resizable(win);
  }

  /// A floating window's size and position, from its frame or declaration.
  place(win) {
    const { visibility, maximized, frame } = win.state.get();
    const element = win.element;
    element.hidden = visibility === MINIMIZED;
    element.classList.toggle("bern-maximized", maximized);
    if (maximized || this.narrow.get()) {
      element.style.left = element.style.top = element.style.width = element.style.height = "";
      return;
    }
    const kind = win.spec;
    const desktop = this.desktop.getBoundingClientRect();
    const width = Math.min(frame?.width ?? kind.width ?? 640, desktop.width);
    const height = Math.min(frame?.height ?? kind.height ?? 480, desktop.height);
    const x = frame?.x ?? Math.max(0, (desktop.width - width) / 2);
    const y = frame?.y ?? Math.max(16, (desktop.height - height) / 3);
    element.style.left = `${x}px`;
    element.style.top = `${y}px`;
    element.style.width = `${width}px`;
    element.style.height = `${height}px`;
    if (kind.minWidth) element.style.minWidth = `${kind.minWidth}px`;
    if (kind.minHeight) element.style.minHeight = `${kind.minHeight}px`;
  }

  setState(win, change) {
    win.state.set({ ...win.state.peek(), ...change });
    requestAnimationFrame(() => win.reportFacts());
  }

  /// Bring a window forward: the page stays at the bottom, floating
  /// windows above it and dialogs above everything.
  activate(win) {
    const order = this.order.peek().filter((candidate) => candidate !== win);
    order.push(win);
    this.order.set(order);
    order.forEach((candidate, index) => {
      const layer = candidate.dialog ? 1000 : candidate.floating ? 10 : 0;
      candidate.root.style.zIndex = String(layer + index);
      candidate.element.classList.toggle("bern-window-active", candidate === win);
    });
    const previous = this.active.peek();
    this.active.set(win);
    previous?.reportFacts();
    win.reportFacts();
  }

  /// A window command, run the same way for the window's own controls and
  /// for every entry; it reports the facts it causes as caused by a command.
  command(win, action, args = {}) {
    const actions = win.actions();
    if (!(actions & (1 << ACTION[action]))) {
      throw new CommandFailure("target_unavailable", `\`${win.id}\` cannot ${action} now`);
    }
    win.commandInFlight = true;
    try {
      const { visibility, maximized, frame } = win.state.peek();
      switch (action) {
        case "minimize":
          win.state.set({ visibility: MINIMIZED, maximized, frame });
          this.activateTop(win);
          break;
        case "restore":
          if (visibility === MINIMIZED) {
            win.state.set({ visibility: VISIBLE, maximized, frame });
            this.activate(win);
          } else win.state.set({ visibility, maximized: false, frame });
          break;
        case "maximize":
          win.state.set({ visibility, maximized: true, frame: this.frameOf(win) });
          break;
        case "move":
        case "resize": {
          const current = this.frameOf(win);
          const next = { ...current, ...args };
          win.state.set({ visibility, maximized, frame: next });
          break;
        }
      }
      win.reportFacts();
    } finally {
      win.commandInFlight = false;
    }
  }

  frameOf(win) {
    return win.frame();
  }

  activateTop(except) {
    const order = this.order.peek().filter((win) => win !== except && win.state.peek().visibility !== MINIMIZED);
    if (order.length) this.activate(order[order.length - 1]);
    else {
      this.active.set(null);
      except.reportFacts();
    }
  }

  /// Drag the title bar: the preview only moves the element; releasing it
  /// commits one `move`.
  draggable(win, handle) {
    handle.addEventListener("pointerdown", (event) => {
      if (event.button !== 0 || event.target.closest("button")) return;
      const { maximized } = win.state.peek();
      if (maximized || this.narrow.peek() || !(win.actions() & (1 << ACTION.move))) return;
      const start = this.frameOf(win);
      const origin = { x: event.clientX, y: event.clientY };
      handle.setPointerCapture(event.pointerId);
      const desktop = this.desktop.getBoundingClientRect();
      const position = (e) => ({
        x: Math.min(desktop.width - 80, Math.max(80 - start.width, start.x + e.clientX - origin.x)),
        y: Math.min(desktop.height - 36, Math.max(0, start.y + e.clientY - origin.y)),
      });
      const move = (e) => {
        const next = position(e);
        win.element.style.left = `${next.x}px`;
        win.element.style.top = `${next.y}px`;
      };
      const up = (e) => {
        handle.removeEventListener("pointermove", move);
        handle.removeEventListener("pointerup", up);
        const next = position(e);
        if (next.x !== start.x || next.y !== start.y) this.command(win, "move", next);
      };
      handle.addEventListener("pointermove", move);
      handle.addEventListener("pointerup", up);
    });
    handle.addEventListener("keydown", (event) => {
      const step = { ArrowLeft: [-8, 0], ArrowRight: [8, 0], ArrowUp: [0, -8], ArrowDown: [0, 8] }[event.key];
      if (!step) return;
      event.preventDefault();
      const frame = this.frameOf(win);
      try {
        if (event.shiftKey) {
          this.command(win, "resize", { width: Math.max(win.spec.minWidth ?? 200, frame.width + step[0]), height: Math.max(win.spec.minHeight ?? 120, frame.height + step[1]) });
        } else this.command(win, "move", { x: frame.x + step[0], y: Math.max(0, frame.y + step[1]) });
      } catch (error) {
        reportError(error);
      }
    });
    handle.tabIndex = 0;
  }

  /// Resize from the edges and corners; releasing commits one `resize`.
  resizable(win) {
    for (const edge of ["n", "e", "s", "w", "ne", "nw", "se", "sw"]) {
      const grip = el("div", { class: `bern-grip bern-grip-${edge}`, "aria-hidden": "true" });
      grip.addEventListener("pointerdown", (event) => {
        if (!(win.actions() & (1 << ACTION.resize))) return;
        event.preventDefault();
        event.stopPropagation();
        grip.setPointerCapture(event.pointerId);
        const start = this.frameOf(win);
        const origin = { x: event.clientX, y: event.clientY };
        const minWidth = win.spec.minWidth ?? 240;
        const minHeight = win.spec.minHeight ?? 160;
        const frame = (e) => {
          const dx = e.clientX - origin.x;
          const dy = e.clientY - origin.y;
          let { x, y, width, height } = start;
          if (edge.includes("e")) width = Math.max(minWidth, start.width + dx);
          if (edge.includes("s")) height = Math.max(minHeight, start.height + dy);
          if (edge.includes("w")) {
            width = Math.max(minWidth, start.width - dx);
            x = start.x + start.width - width;
          }
          if (edge.includes("n")) {
            height = Math.max(minHeight, start.height - dy);
            y = Math.max(0, start.y + start.height - height);
          }
          return { x, y, width, height };
        };
        const move = (e) => {
          const next = frame(e);
          Object.assign(win.element.style, { left: `${next.x}px`, top: `${next.y}px`, width: `${next.width}px`, height: `${next.height}px` });
        };
        const up = (e) => {
          grip.removeEventListener("pointermove", move);
          grip.removeEventListener("pointerup", up);
          this.command(win, "resize", frame(e));
        };
        grip.addEventListener("pointermove", move);
        grip.addEventListener("pointerup", up);
      });
      win.element.append(grip);
    }
  }

  /// Request closing through the close protocol: close now, or show the
  /// window's confirmation and wait for a decision.
  close(win) {
    try {
      return this.requestClose(win);
    } catch (error) {
      win.fail(error);
      return null;
    }
  }

  requestClose(win) {
    const check = win.closeCheck;
    const needs = check ? untrack(check.needsConfirmation) : false;
    const reader = host.requestClose(win, needs, !!check?.save);
    if (reader.u8() !== 0) throw new CommandFailure(reader.text(), reader.text());
    if (reader.u8() === 0) {
      this.finishClose(win);
      return { outcome: 0, window: win.id };
    }
    const confirmation = reader.text();
    const draftRevision = reader.u64();
    const pending = {
      id: confirmation, draftRevision, message: untrack(check.message), canSave: !!check.save,
    };
    win.pendingClose.set(pending);
    return { outcome: 2, window: win.id, ...pending };
  }

  /// Decide a pending confirmation, as its dialog does.
  confirmClose(win, confirmation, decision, done) {
    const reader = host.confirmClose(win, confirmation, decision);
    if (reader.u8() !== 0) {
      done(new CommandFailure(reader.text(), reader.text()));
      return;
    }
    const step = reader.u8();
    win.pendingClose.set(null);
    if (step === 0) {
      this.finishClose(win);
      done(null, { outcome: 0, window: win.id });
    } else if (step === 2) {
      done(null, { outcome: 1, window: win.id, reason: "cancelled" });
    } else {
      win.closeCheck.save((failure) => {
        if (failure) done(null, { outcome: 1, window: win.id, reason: `save_failed: ${failure.code}` });
        else {
          this.finishClose(win);
          done(null, { outcome: 0, window: win.id });
        }
      });
    }
  }

  finishClose(win) {
    win.scope.dispose();
    win.root.remove();
    this.windows.delete(win.spec.id);
    this.order.set(this.order.peek().filter((candidate) => candidate !== win));
    host.windowClosed(win.kind, win.generation);
    if (this.active.peek() === win) this.activateTop(win);
    // Focus returns to what opened the dialog.
    if (win.dialog && win.returnFocus?.isConnected) win.returnFocus.focus();
  }

  /// A modal alert over its own window; Escape chooses `cancel` and Tab
  /// stays inside.
  modal(win, title, detail, buttons, cancel, initial) {
    const heading = el("h2", { class: "bern-alert-title", id: `bern-alert-${win.spec.id}` }, title);
    const box = el("div", {
      class: "bern-alert", role: "alertdialog", "aria-modal": "true", "aria-labelledby": `bern-alert-${win.spec.id}`,
    }, heading, el("p", { class: "bern-alert-detail" }, detail), el("div", { class: "bern-alert-buttons" }, ...buttons));
    const scrim = el("div", { class: "bern-scrim" }, box);
    scrim.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && !event.isComposing) {
        event.preventDefault();
        cancel.click();
      } else if (event.key === "Tab") trapFocus(box, event);
    });
    win.element.append(scrim);
    queueMicrotask(() => initial.focus());
    onCleanup(() => scrim.remove());
  }

  /// The close confirmation: Save, Don't Save and Cancel, or Discard and
  /// Cancel, with the main choice last.
  dialog(win) {
    const pending = win.pendingClose.get();
    if (!pending) return;
    const choice = (label, decision, kind) => {
      const node = el("button", { class: `bern-button bern-button-${kind}`, type: "button" }, tr(label));
      node.addEventListener("click", () => this.confirmClose(win, pending.id, decision, (failure) => {
        if (failure) win.fail(failure);
      }));
      return node;
    };
    const cancel = choice("bern.close.cancel", 2, "standard");
    const detail = tr(pending.canSave ? "bern.close.save_detail" : "bern.close.discard_detail");
    if (pending.canSave) {
      const save = choice("bern.close.save", 0, "prominent");
      const discard = choice("bern.close.dont_save", 1, "destructive-plain");
      discard.classList.add("bern-alert-aside");
      this.modal(win, pending.message, detail, [discard, cancel, save], cancel, save);
    } else {
      this.modal(win, pending.message, detail, [cancel, choice("bern.close.discard", 1, "destructive")], cancel, cancel);
    }
  }

  /// A failed command shows an alert in its window.
  alert(win) {
    const failure = win.failure.get();
    if (!failure) return;
    const ok = el("button", { class: "bern-button bern-button-prominent", type: "button" }, tr("bern.alert.ok"));
    ok.addEventListener("click", () => win.failure.set(null));
    this.modal(win, tr("bern.alert.failed"), failure.message || failure.code || String(failure), [ok], ok, ok);
  }
}

/// Drafts of one session type in one window instance. Fields are cells;
/// writes go through Rust and apply only when accepted.
class SessionState {
  constructor(win, index, type, reader) {
    for (const [field, spec] of type.fields.entries()) {
      const value = cell(spec.codec[0](reader));
      this[spec.name] = {
        get: () => value.get(),
        peek: () => value.peek(),
        set: (next) => {
          if (win.set(index, field, spec.codec[1], next)) value.set(next);
        },
      };
    }
  }
}

let manager = null;

/// Run commands of the window that owns a control.
export function windowOf(ctx) {
  return ctx.win;
}

export function openWindow(id) {
  manager.open(id);
}

export function closeWindow(id) {
  const win = manager.windows.get(id);
  if (win) manager.close(win);
}

// MARK: - Startup

/// Start the page: load the runtime, start the engine, open the windows
/// that open at launch.
export async function start(app) {
  icons = app.icons;
  supportedLanguages = app.languages;
  allStrings = app.strings;
  const initial = chooseLanguage(app.languages, app.defaultLanguage);
  languageCell.set(initial);
  stringsCell.set(app.strings[initial] ?? app.strings[app.defaultLanguage] ?? {});
  effect(() => {
    document.documentElement.lang = languageCell.get();
    document.title = tr("app.name");
  });
  engine = app.engine;
  const root = document.getElementById("bern-host");
  try {
    await host.load(app.wasm);
    if (app.mode === "ui") {
      // The channel credential travels in the address fragment, which the
      // browser never sends to the server.
      const token = new URLSearchParams(location.hash.slice(1)).get("token") ?? "";
      host.startRemote(token);
      linkBanner(root);
      const connected = new Promise((resolve, reject) => {
        link.firstConnect = { resolve, reject };
      });
      link.open();
      await connected;
    } else {
      host.start(app.fingerprint, app.resources);
    }
    media.attach();
  } catch (error) {
    root.replaceChildren(el("div", { class: "bern-startup-failure", role: "alert" },
      el("h1", {}, tr("bern.startup.failed")), el("p", {}, String(error.message ?? error))));
    throw error;
  }
  manager = new WindowManager(app, root);
  for (const kind of app.windows) {
    if (kind.initial) manager.open(kind.id);
  }
  // Development handle for inspecting the running page.
  window.bern = { host, manager, link, controls: controlEntries };
}

/// Dispatch an engine command for a control in `ctx`'s window.
export function dispatch(ctx, command, encode, completion) {
  host.dispatch(command, encode, ctx?.win ?? null, completion);
}
