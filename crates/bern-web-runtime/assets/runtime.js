// DOM and connection bridge. Typed local commands and theme tokens live in WASM.
import init, { WebSession } from "./pkg/bern_web_runtime.js";

const statusElement = document.getElementById("connection-status");
const mainElement = document.getElementById("main-content");
const rootElement = document.getElementById("app-root");
const noticeElement = document.getElementById("notice");
const bindings = [];
const editors = [];
const engineButtons = [];
const pending = new Map();
const REQUEST_TIMEOUT = 15000;
let app;
let session;
let sessionSnapshot;
let engineSnapshot = null;
let engineId = null;
let availableCommands = new Set();
let socket = null;
let reconnectTimer = null;
let welcomeTimer = null;
let reconnectAttempt = 0;
let ready = false;
let stopped = false;
let noticeTimer = null;
let endpoint;
let connectionState = "connecting";

function notify(message, { error = false, persistent = false } = {}) {
  clearTimeout(noticeTimer);
  noticeElement.textContent = message;
  noticeElement.dataset.tone = error ? "error" : "neutral";
  noticeElement.hidden = !message;
  if (message && !persistent) {
    noticeTimer = setTimeout(() => { noticeElement.hidden = true; }, 7000);
  }
}

function visibleError(error) {
  if (error?.message && /[\u3400-\u9fff]/u.test(error.message)) return error.message;
  const messages = {
    unknown_command: "此操作暂不可用，请更新页面后重试。",
    invalid_params: "操作参数无效，请检查后重试。",
    wrong_engine: "连接已改变，请确认当前内容后重试。",
    invalid_request: "无法完成此操作，请重新打开页面。",
    result_unknown: "操作结果尚未确认，请确认当前内容后再试。",
  };
  return messages[error?.code] ?? "操作未能完成，请稍后再试。";
}

function setConnection(state, message) {
  connectionState = state;
  statusElement.dataset.state = state;
  statusElement.textContent = message;
  mainElement.dataset.remoteStale = String(!ready && engineSnapshot !== null);
  updateButtons();
}

function updateButtons() {
  for (const { element, command } of engineButtons) {
    const busy = [...pending.values()].some((request) => request.command === command);
    element.disabled = !ready || !availableCommands.has(command) || busy;
    element.setAttribute("aria-busy", String(busy));
    element.title = !ready ? "连接恢复后可用" : !availableCommands.has(command) ? "此操作暂不可用" : "";
  }
}

function displayValue(value) {
  if (value === null || value === undefined || value === "") return "—";
  if (typeof value === "boolean") return value ? "是" : "否";
  return String(value);
}

function applyBindings(scope) {
  const snapshot = scope === "session" ? sessionSnapshot : engineSnapshot;
  for (const { element, field } of bindings) {
    if (field.scope !== scope) continue;
    const text = displayValue(snapshot?.values[field.id]);
    if (element.textContent !== text) element.textContent = text;
  }
}

function synchronizeEditors() {
  for (const editor of editors) {
    const value = sessionSnapshot.values[editor.field.id];
    if (editor.element.type === "checkbox") {
      editor.element.checked = value;
    } else if (!editor.composing && editor.element.value !== value) {
      // Normal typing already has the accepted value. Only programmatic
      // command updates/rejections write it, preserving focus and selection.
      const input = editor.element;
      const focused = document.activeElement === input;
      const start = input.selectionStart;
      const end = input.selectionEnd;
      const direction = input.selectionDirection;
      input.value = value;
      if (focused && start !== null && end !== null) {
        input.setSelectionRange(Math.min(start, value.length), Math.min(end, value.length), direction);
      }
    }
  }
}

function dispatchSession(command, target, payload = {}) {
  let result;
  try {
    if (typeof command !== "string" || typeof target !== "string") throw new Error("invalid command target");
    result = JSON.parse(session.dispatch(command, target, JSON.stringify(payload)));
  } catch (error) {
    console.warn("Bern session dispatch failed", error);
    result = { ok: false, error: { code: "invalid_params", message: "操作参数无效" } };
  }
  // The semantic draft is always read back from Rust, including after failure.
  sessionSnapshot = JSON.parse(session.snapshot());
  synchronizeEditors();
  applyBindings("session");
  const targetEditor = editors.find((editor) => editor.targetId === target);
  if (targetEditor) {
    fieldFeedback(targetEditor, result);
    if (result.ok) {
      for (const editor of editors) {
        if (editor.field.id === targetEditor.field.id) fieldFeedback(editor, result);
      }
    }
  }
  return result;
}

function fieldFeedback(editor, result) {
  editor.element.setAttribute("aria-invalid", String(!result.ok));
  editor.error.textContent = result.ok ? "" : visibleError(result.error);
  editor.error.hidden = result.ok;
}

function elementFor(node) {
  let element;
  switch (node.kind) {
    case "column":
    case "row": {
      element = document.createElement("div");
      element.className = `bern-${node.kind}${node.padding > 0 ? " surface-panel" : ""}`;
      element.style.setProperty("--layout-gap", `${node.gap}px`);
      element.style.setProperty("--layout-padding", `${node.padding}px`);
      for (const [index, child] of node.children.entries()) {
        const childElement = elementFor(child);
        if (node.padding > 0 && index === 0 && child.kind === "text" && child.content.kind === "literal") {
          childElement.classList.add("section-label");
        }
        element.append(childElement);
      }
      break;
    }
    case "text": {
      element = document.createElement("p");
      element.className = "bern-text";
      if (node.content.kind === "literal") {
        element.textContent = node.content.value;
      } else {
        const field = node.content.field;
        element.classList.add(`value-${field.kind}`);
        element.dataset.scope = field.scope;
        element.textContent = "—";
        bindings.push({ element, field });
      }
      break;
    }
    case "button": {
      element = document.createElement("button");
      element.className = "bern-button";
      element.type = "button";
      element.textContent = node.label;
      element.disabled = true;
      element.addEventListener("click", () => { void dispatchEngine(node.command); });
      engineButtons.push({ element, command: node.command });
      break;
    }
    case "text_input":
    case "checkbox": {
      element = document.createElement("div");
      const checkbox = node.kind === "checkbox";
      element.className = checkbox ? "bern-checkbox-field" : "bern-input-field";
      const label = document.createElement("label");
      label.htmlFor = `bern-input-${node.id}`;
      label.textContent = node.label;
      const input = document.createElement("input");
      input.id = label.htmlFor;
      input.name = node.field.id;
      input.type = checkbox ? "checkbox" : "text";
      input.className = checkbox ? "bern-checkbox" : "bern-input";
      const error = document.createElement("p");
      error.id = `bern-error-${node.id}`;
      error.className = "field-error";
      error.hidden = true;
      error.setAttribute("role", "status");
      input.setAttribute("aria-describedby", error.id);
      const editor = { targetId: node.id, element: input, field: node.field, composing: false, error };
      editors.push(editor);
      if (checkbox) {
        const control = document.createElement("div");
        control.className = "checkbox-control";
        control.append(input, label);
        element.append(control, error);
        input.addEventListener("change", () => {
          dispatchSession("session.toggle", node.id);
        });
      } else {
        element.append(label, input, error);
        const commit = () => dispatchSession("session.edit", node.id, { value: input.value });
        input.addEventListener("compositionstart", () => { editor.composing = true; });
        input.addEventListener("compositionend", () => { editor.composing = false; commit(); });
        input.addEventListener("input", (event) => {
          // Composition is an ephemeral native edit buffer. Only the command
          // below commits a semantic draft in the Rust session.
          if (!editor.composing && !event.isComposing) commit();
        });
      }
      break;
    }
    default:
      throw new Error(`Unsupported component: ${node.kind}`);
  }
  element.dataset.bernId = node.id;
  return element;
}

function parseRevision(value) {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]{0,19})$/u.test(value)) {
    throw new Error("Invalid snapshot revision");
  }
  const revision = BigInt(value);
  if (revision > 18446744073709551615n) throw new Error("Snapshot revision is out of range");
  return revision;
}

function accepts(kind, value) {
  if (kind === "integer") return Number.isInteger(value) && value >= -2147483648 && value <= 2147483647;
  if (kind === "boolean") return typeof value === "boolean";
  return typeof value === "string" && new TextEncoder().encode(value).length <= 4096;
}

function acceptSnapshot(snapshot, allowNewEngine = false) {
  if (!snapshot || typeof snapshot.engine_id !== "string" || !snapshot.values || Array.isArray(snapshot.values)
      || typeof snapshot.values !== "object") throw new Error("Invalid engine snapshot");
  if (!allowNewEngine && snapshot.engine_id !== engineId) throw new Error("Snapshot belongs to a different engine");
  const revision = parseRevision(snapshot.revision);
  for (const field of app.fields) {
    if (field.scope === "engine" && (!Object.hasOwn(snapshot.values, field.id) || !accepts(field.kind, snapshot.values[field.id]))) {
      throw new Error(`Invalid engine field: ${field.id}`);
    }
  }
  if (engineSnapshot?.engine_id === snapshot.engine_id) {
    const previous = parseRevision(engineSnapshot.revision);
    if (revision < previous) return;
    if (revision === previous) {
      for (const field of app.fields.filter((field) => field.scope === "engine")) {
        if (engineSnapshot.values[field.id] !== snapshot.values[field.id]) throw new Error("Conflicting snapshot revision");
      }
    }
  }
  engineSnapshot = snapshot;
  applyBindings("engine");
}

function finishRequest(requestId, result) {
  const request = pending.get(requestId);
  if (!request) return;
  clearTimeout(request.timer);
  pending.delete(requestId);
  request.resolve(result);
  updateButtons();
}

function abandonPending() {
  const hadPending = pending.size > 0;
  for (const requestId of [...pending.keys()]) {
    finishRequest(requestId, { ok: false, request_id: requestId,
      error: { code: "result_unknown", message: "连接已中断，操作结果尚未确认。" } });
  }
  if (hadPending) notify("连接已中断，操作结果尚未确认。请确认当前内容后再试。", { error: true, persistent: true });
}

function dispatchEngine(command) {
  if (!ready || socket?.readyState !== WebSocket.OPEN || !availableCommands.has(command)) {
    const result = { ok: false, error: { code: "unavailable", message: "连接恢复后可继续操作。" } };
    notify(result.error.message, { error: true });
    return Promise.resolve(result);
  }
  if (!app.commands.some((entry) => entry.id === command)) {
    return Promise.resolve({ ok: false, error: { code: "unknown_command", message: "未登记的操作" } });
  }
  const requestId = crypto.randomUUID();
  const request = { protocol_version: 1, request_id: requestId, engine_id: engineId, command, params: {} };
  const promise = new Promise((resolve) => {
    const timer = setTimeout(() => {
      finishRequest(requestId, { ok: false, request_id: requestId,
        error: { code: "result_unknown", message: "操作结果尚未确认，请确认当前内容后再试。" } });
      notify("操作结果尚未确认，请确认当前内容后再试。", { error: true, persistent: true });
    }, REQUEST_TIMEOUT);
    pending.set(requestId, { command, engineId, timer, resolve });
  });
  updateButtons();
  try {
    socket.send(JSON.stringify(request));
  } catch (error) {
    console.warn("Bern command send failed", error);
    finishRequest(requestId, { ok: false, request_id: requestId,
      error: { code: "result_unknown", message: "操作结果尚未确认，请检查连接。" } });
    notify("操作结果尚未确认，请检查连接。", { error: true, persistent: true });
  }
  return promise;
}

function receiveMessage(event, source) {
  if (source !== socket) return;
  try {
    const message = JSON.parse(event.data);
    switch (message.type) {
      case "welcome": {
        if (message.protocol_version !== 1 || typeof message.engine_id !== "string" || !message.engine_id
            || !Array.isArray(message.commands) || message.snapshot?.engine_id !== message.engine_id) {
          throw new Error("Incompatible welcome message");
        }
        const commandIds = message.commands.map((command) => {
          if (typeof command.id !== "string" || typeof command.label !== "string") throw new Error("Invalid command catalog");
          return command.id;
        });
        if (app.commands.some((command) => !commandIds.includes(command.id))) throw new Error("Required command is unavailable");
        if (engineId !== null && engineId !== message.engine_id) abandonPending();
        acceptSnapshot(message.snapshot, true);
        engineId = message.engine_id;
        availableCommands = new Set(commandIds);
        ready = true;
        reconnectAttempt = 0;
        clearTimeout(welcomeTimer);
        setConnection("connected", "已连接");
        break;
      }
      case "snapshot":
        if (!ready) throw new Error("Snapshot before welcome");
        acceptSnapshot(message.snapshot);
        break;
      case "result": {
        const request = pending.get(message.request_id);
        if (!request) break;
        if (message.engine_id !== request.engineId || typeof message.ok !== "boolean"
            || (message.ok && message.error !== null)
            || (!message.ok && (typeof message.error?.code !== "string" || typeof message.error?.message !== "string"))) {
          throw new Error("Invalid command result");
        }
        parseRevision(message.revision);
        finishRequest(message.request_id, message);
        if (!message.ok) notify(visibleError(message.error), { error: true });
        break;
      }
      default:
        throw new Error("Unknown server message");
    }
  } catch (error) {
    console.error("Bern protocol error", error);
    ready = false;
    stopped = true;
    clearTimeout(welcomeTimer);
    abandonPending();
    setConnection("error", "连接版本不兼容");
    notify("服务与此页面暂不兼容，请更新页面后重试。", { error: true, persistent: true });
    source.close(4002, "Protocol mismatch");
  }
}

function scheduleReconnect() {
  if (stopped || reconnectTimer !== null) return;
  const delay = Math.min(1000 * 2 ** Math.min(reconnectAttempt++, 4), 15000);
  reconnectTimer = setTimeout(() => { reconnectTimer = null; connect(); }, delay + Math.random() * 200);
}

function connect() {
  if (stopped) return;
  ready = false;
  setConnection("connecting", reconnectAttempt ? "正在重新连接" : "正在连接");
  let next;
  try {
    next = new WebSocket(endpoint);
  } catch (error) {
    console.warn("Bern connection failed", error);
    setConnection("disconnected", "暂时无法连接");
    scheduleReconnect();
    return;
  }
  socket = next;
  welcomeTimer = setTimeout(() => {
    if (!ready && next === socket) next.close();
  }, 10000);
  next.addEventListener("message", (event) => receiveMessage(event, next));
  next.addEventListener("close", () => {
    if (next !== socket) return;
    clearTimeout(welcomeTimer);
    ready = false;
    abandonPending();
    if (!stopped) {
      setConnection("disconnected", "连接中断，正在重连");
      scheduleReconnect();
    }
  });
  next.addEventListener("error", () => {
    // The close event owns reconnection so a failed connection never creates
    // two reconnect loops or replays any accepted command.
    if (next === socket && !stopped) setConnection("disconnected", "暂时无法连接");
  });
}

function connectionUrl(config) {
  if (typeof config.endpoint !== "string" || !config.endpoint) throw new Error("Missing endpoint");
  const url = new URL(config.endpoint, location.href);
  if (url.protocol === "http:") url.protocol = "ws:";
  if (url.protocol === "https:") url.protocol = "wss:";
  if (!["ws:", "wss:"].includes(url.protocol) || url.username || url.password) throw new Error("Invalid endpoint");
  return url.href;
}

async function loadJson(path) {
  const response = await fetch(new URL(path, import.meta.url), { cache: "no-store" });
  if (!response.ok) throw new Error(`Failed to load ${path}: ${response.status}`);
  return response.json();
}

async function start() {
  const [, description, connection] = await Promise.all([init(), loadJson("./ui.json"), loadJson("./connection.json")]);
  app = description;
  session = new WebSession(JSON.stringify(app), crypto.randomUUID());
  sessionSnapshot = JSON.parse(session.snapshot());
  const appearance = matchMedia("(prefers-color-scheme: dark)");
  const applyTheme = () => {
    const theme = JSON.parse(session.theme(appearance.matches ? "dark" : "light"));
    for (const [name, value] of Object.entries(theme.tokens)) {
      document.documentElement.style.setProperty(`--bern-${name}`, value);
    }
    document.documentElement.dataset.theme = theme.mode;
    document.documentElement.style.colorScheme = theme.mode;
  };
  applyTheme();
  appearance.addEventListener("change", applyTheme);
  document.title = app.title;
  document.getElementById("app-title").textContent = app.title;
  rootElement.append(elementFor(app.root));
  synchronizeEditors();
  applyBindings("session");
  mainElement.setAttribute("aria-busy", "false");
  endpoint = connectionUrl(connection);

  // A small, explicit inspection/automation surface. Returned data are fresh
  // JSON values; callers cannot mutate the authoritative session by reference.
  Object.defineProperty(window, "Bern", { configurable: false, writable: false, value: Object.freeze({
    session: Object.freeze({
      catalog: () => JSON.parse(session.catalog()),
      snapshot: () => JSON.parse(session.snapshot()),
      dispatch: dispatchSession,
    }),
    engine: Object.freeze({
      snapshot: () => engineSnapshot === null ? null : JSON.parse(JSON.stringify(engineSnapshot)),
      commands: () => [...availableCommands],
      connection: () => ({ state: connectionState, engine_id: engineId, pending: pending.size }),
      dispatch: dispatchEngine,
    }),
  }) });
  connect();

  // A BFCache restoration keeps the same local draft/session and explicitly
  // reconnects its execution endpoint. A real reload creates a new session.
  window.addEventListener("pagehide", () => {
    stopped = true;
    ready = false;
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
    clearTimeout(welcomeTimer);
    abandonPending();
    socket?.close();
  });
  window.addEventListener("pageshow", (event) => {
    if (event.persisted) { stopped = false; reconnectAttempt = 0; connect(); }
  });
}

start().catch((error) => {
  console.error("Bern startup failed", error);
  stopped = true;
  ready = false;
  mainElement.setAttribute("aria-busy", "false");
  document.getElementById("app-title").textContent = "暂时无法打开应用";
  setConnection("error", "加载未完成");
  notify("页面未能完整加载，请检查连接后刷新重试。", { error: true, persistent: true });
});
