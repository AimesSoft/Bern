import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

// Evaluate the actual runtime in a small DOM fixture. These tests exercise
// reactive effects and lifetimes without starting a browser or a WASM app.
function runtime(search = "") {
  class Element {
    constructor(tag, nodeType = 1) {
      this.tagName = tag;
      this.nodeType = nodeType;
      this.childNodes = [];
      this.attributes = {};
      this.events = {};
      this.style = { setProperty(name, value) { this[name] = value; } };
      this.classes = new Set();
      this.classList = {
        add: (...names) => names.forEach((name) => this.classes.add(name)),
        toggle: (name, enabled) => enabled ? this.classes.add(name) : this.classes.delete(name),
      };
    }
    set className(value) { this.classes = new Set(value.split(/\s+/)); }
    get className() { return [...this.classes].join(" "); }
    setAttribute(name, value) { this.attributes[name] = String(value); }
    removeAttribute(name) { delete this.attributes[name]; }
    addEventListener(name, callback) { (this.events[name] ??= []).push(callback); }
    removeEventListener(name, callback) { this.events[name] = (this.events[name] ?? []).filter(one => one !== callback); }
    get children() { return this.childNodes.filter((child) => child.nodeType === 1); }
    querySelectorAll(selector) {
      const all = this.children.flatMap((child) => [child, ...child.querySelectorAll(selector)]);
      return all.filter((child) => selector === "[data-bern-item]" && child.attributes["data-bern-item"] !== undefined);
    }
    closest(selector) {
      if (selector === ".bern-scroll" && this.classes.has("bern-scroll")) return this;
      return this.parentElement?.closest(selector) ?? null;
    }
    get clientWidth() { return this.rect?.width ?? 100; }
    get clientHeight() { return this.rect?.height ?? 40; }
    scrollTo(options) {
      this.lastScroll = options;
      if (options.top !== undefined) this.scrollTop = options.top;
      if (options.left !== undefined) this.scrollLeft = options.left;
    }
    getBoundingClientRect() { return this.rect ?? { left: 0, top: 0, width: 100, height: 40 }; }
    remove() {
      if (this.parentNode) this.parentNode.childNodes = this.parentNode.childNodes.filter((child) => child !== this);
      this.parentNode = this.parentElement = null;
    }
    append(...children) {
      for (const child of children) {
        if (child.nodeType === 11) this.append(...child.childNodes.splice(0));
        else { this.childNodes.push(child); child.parentNode = this; child.parentElement = this.nodeType === 1 ? this : null; }
      }
    }
    prepend(child) { this.childNodes.unshift(child); child.parentElement = this; }
  }
  const probes = [];
  class Probe {
    constructor() { probes.push(this); }
    load(width, height) {
      this.naturalWidth = width;
      this.naturalHeight = height;
      this.onload?.();
    }
  }
  const queries = new Map();
  const timers = [];
  const frames = [];
  const microtasks = [];
  const resizeObservers = [];
  const mutationObservers = [];
  class Observer {
    constructor(callback, instances) { this.callback = callback; this.targets = new Set(); instances.push(this); }
    observe(target) { this.targets.add(target); }
    unobserve(target) { this.targets.delete(target); }
    disconnect() { this.targets.clear(); this.disconnected = true; }
  }
  const globals = {
    TextEncoder, TextDecoder, URLSearchParams, performance,
    navigator: { platform: "MacIntel" }, location: { search }, Image: Probe,
    Node: { ELEMENT_NODE: 1, DOCUMENT_POSITION_FOLLOWING: 4 },
    document: {
      listeners: {},
      addEventListener(name, callback) { (this.listeners[name] ??= []).push(callback); },
      querySelector: () => null,
      createElement: (tag) => new Element(tag),
      createElementNS: (_, tag) => new Element(tag),
      createDocumentFragment: () => new Element("fragment", 11),
      createComment: () => new Element("comment", 8),
    },
    ResizeObserver: class extends Observer {
      constructor(callback) { super(callback, resizeObservers); }
      observe(target) {
        assert.equal(target.nodeType, 1, "ResizeObserver requires an Element");
        super.observe(target);
      }
    },
    MutationObserver: class extends Observer { constructor(callback) { super(callback, mutationObservers); } },
    matchMedia(query) {
      if (!queries.has(query)) queries.set(query, {
        matches: false,
        addEventListener(_, callback) { this.change = callback; },
      });
      return queries.get(query);
    },
    getComputedStyle: (node) => ({ direction: "ltr", paddingTop: node.style.paddingBlock ?? "0px", paddingLeft: node.style.paddingInline ?? "0px" }),
    requestAnimationFrame: (callback) => { frames.push(callback); return frames.length; },
    cancelAnimationFrame() {},
    setTimeout: (callback, delay) => { timers.push({ callback, delay }); return timers.length; },
    clearTimeout(id) { if (timers[id - 1]) timers[id - 1].cancelled = true; },
    queueMicrotask: (callback) => microtasks.push(callback),
    reportError(error) { throw error; },
  };
  const context = vm.createContext(globals);
  for (const [name, extra] of [["bern", []], ["features", ["transitionFrom"]]]) {
    const source = readFileSync(new URL(`../templates/${name}.js`, import.meta.url), "utf8");
    const exported = [...source.matchAll(/^export (?:async )?(?:class|function|const) (\w+)/gm)].map((match) => match[1]);
    const plain = source.replace(/^import .+;$/gm, "").replace(/^export /gm, "");
    vm.runInContext(`globalThis.${name === "bern" ? "B" : "F"} = (() => { ${plain}\nreturn { ${[...exported, ...extra].join(", ")} }; })();`, context, { filename: `${name}.js` });
  }
  const flushMicrotasks = () => { while (microtasks.length) microtasks.shift()(); };
  return { B: context.B, F: context.F, Element, document: globals.document, probes, queries, timers, frames, flushMicrotasks, resizeObservers, mutationObservers };
}

test("neutral sidebar selection pairs text with its replacement fill through hover and selection changes", () => {
  const { B, F, Element } = runtime();
  const row = new Element("div");
  row.className = "bern-sidebar-item";
  const selected = B.cell(false);
  const scope = new B.Scope();
  B.within(scope, () => F.applyStyle(row, {
    base: {},
    selected: { fill: { role: "fill.strong" } },
    hover: { background: { role: "fill.subtle" } },
  }, { selected: () => selected.get() }));
  const hover = (value) => row.events[value ? "pointerenter" : "pointerleave"][0]({ pointerType: "mouse" });
  assert.equal(row.style.color, "");
  hover(true);
  assert.equal(row.style.background, "var(--bern-fill-subtle)");
  assert.equal(row.style.color, "var(--bern-text-primary)");
  selected.set(true);
  assert.equal(row.style.background, "var(--bern-fill-strong)");
  assert.equal(row.style.color, "var(--bern-text-primary)");
  assert.ok(row.classes.has("bern-styled-foreground"));
  hover(false);
  assert.equal(row.style.color, "var(--bern-text-primary)");
  selected.set(false);
  assert.equal(row.style.background, "");
  assert.equal(row.style.color, "");
  assert.equal(row.classes.has("bern-styled-foreground"), false);
  scope.dispose();
  const css = readFileSync(new URL("../templates/bern.css", import.meta.url), "utf8");
  assert.match(css, /\.bern-sidebar-item\[aria-selected="true"\] \.bern-icon,[^{]+\{[^}]*color:\s*inherit;/);
});

test("fill pairing preserves accent selections and an explicit foreground", () => {
  const { B, F, Element } = runtime();
  for (const [fill, expected] of [
    ["fill.subtle", "text-primary"], ["fill.strong", "text-primary"],
    ["surface.canvas", "text-primary"], ["surface.panel", "text-primary"], ["surface.raised", "text-primary"],
    ["accent", "on-accent"], ["selection.background", "selection-foreground"],
  ]) {
    const row = new Element("div");
    const scope = new B.Scope();
    B.within(scope, () => F.applyStyle(row, { base: {}, selected: { fill: { role: fill } } }, { selected: () => true }));
    assert.equal(row.style.color, `var(--bern-${expected})`, fill);
    scope.dispose();
  }
  const row = new Element("div");
  const scope = new B.Scope();
  B.within(scope, () => F.applyStyle(row, {
    base: { foreground: { role: "text.secondary" } },
    selected: { fill: { role: "fill.strong" } },
  }, { selected: () => true }));
  assert.equal(row.style.color, "var(--bern-text-secondary)");
  scope.dispose();
});

test("template images derive their missing side from the source aspect ratio", () => {
  const { B, F, Element, probes } = runtime();
  const root = new Element("root");
  const size = B.cell(84);
  const scope = new B.Scope();
  B.within(scope, () => F.Image(root, {}, { rendering: "template", height: () => size.get(), url: () => "wordmark.png", fit: "fill" }));
  const image = root.childNodes[0];
  probes[0].load(600, 100);
  assert.equal(image.style.width, "504px");
  assert.equal(image.style.height, "84px");
  assert.equal(image.style.maskSize, "100% 100%");
  size.set(72);
  assert.equal(image.style.width, "432px");
  assert.equal(image.style.height, "72px");
  scope.dispose();
  assert.equal(probes[0].onload, null);
  const css = readFileSync(new URL("../templates/features.css", import.meta.url), "utf8");
  assert.match(css, /\.bern-image-template\s*\{[^}]*background:\s*currentColor/);
});

test("content capsules preserve their children and bind their declared height", () => {
  const { B, F, Element } = runtime();
  const root = new Element("root");
  const height = B.cell(30);
  const enabled = B.cell(true);
  const scope = new B.Scope();
  const child = new Element("span");
  let activations = 0;
  B.within(scope, () => F.ContentButton(root, { comp: "demo" }, {
    id: "capsule", label: () => "Enable", shape: "capsule", height: () => height.get(), enabled: () => enabled.get(),
  }, () => activations++, (node) => node.append(child)));
  const button = root.childNodes[0];
  assert.ok(button.classes.has("bern-control-capsule"));
  assert.equal(button.style.borderRadius, "999px");
  assert.equal(button.style.height, "30px");
  assert.equal(button.childNodes[0], child);
  assert.equal(button.attributes["aria-label"], "Enable");
  height.set(42);
  enabled.set(false);
  assert.equal(button.style.height, "42px");
  assert.equal(button.disabled, true);
  const control = B.controlEntries().find((entry) => entry.target() === "demo.capsule");
  control.actions.activate();
  assert.equal(activations, 1);
  scope.dispose();
  assert.equal(B.controlEntries().length, 0);
});

test("declared sidebar widths stay overridable by compact layout CSS", () => {
  const { B, F, Element } = runtime();
  const root = new Element("root");
  const width = B.cell(224);
  F.SplitView(root, {}, { selection: B.cell(0), width: () => width.get(), titled: () => false }, () => {}, () => {});
  const sidebar = root.childNodes[0].childNodes[0];
  assert.equal(sidebar.style.width, undefined);
  assert.equal(sidebar.style["--bern-sidebar-width"], "224px");
  width.set(260);
  assert.equal(sidebar.style["--bern-sidebar-width"], "260px");
  const css = readFileSync(new URL("../templates/bern.css", import.meta.url), "utf8") + readFileSync(new URL("../templates/features.css", import.meta.url), "utf8");
  assert.match(css, /width:\s*var\(--bern-sidebar-width/);
  assert.match(css, /@container\s*\(max-width:\s*767px\)\s*\{\s*\.bern-sidebar\s*\{\s*width:\s*100%/);
});

test("edge grow keeps its opacity and anchors the scaling edge", () => {
  const { F, Element } = runtime();
  const node = new Element("div");
  const from = F.transitionFrom({ kind: "grow", edge: "top" }, node);
  assert.equal(from.opacity, 1);
  assert.equal(from.sy, 0);
  assert.equal(from.sx, 1);
  assert.equal(node.style.transformOrigin, "center top");
});

test("application reduce-motion preference follows state and its mounted lifetime", () => {
  const { B, F, Element, queries } = runtime();
  const root = new Element("root");
  const preference = B.cell(false);
  const scope = new B.Scope();
  B.within(scope, () => F.Decorate(root, {}, { reduceMotion: () => preference.get() }, () => {}));
  assert.equal(F.reduceMotion(), false);
  preference.set(true);
  assert.equal(F.reduceMotion(), true);
  preference.set(false);
  assert.equal(F.reduceMotion(), false);
  preference.set(true);
  scope.dispose();
  assert.equal(F.reduceMotion(), false);
  const system = queries.get("(prefers-reduced-motion: reduce)");
  system.matches = true;
  system.change();
  assert.equal(F.reduceMotion(), true);
});

test("appear delay uses seconds and is bypassed by reduced motion", () => {
  for (const reduced of [false, true]) {
    const { B, F, Element, timers } = runtime(reduced ? "?motion=reduce" : "");
    const root = new Element("root");
    B.structural(true, () => F.Decorate(root, {}, {
      appear: { kind: "fade", motion: [0.2, 1] }, appearDelay: 0.25,
    }, (p) => p.append(new Element("div"))));
    assert.equal(timers.length, reduced ? 0 : 1);
    if (!reduced) assert.equal(timers[0].delay, 250);
  }
});

test("fragment reflow observes its mounted parent and animates relocated rows", () => {
  const { B, F, Element, flushMicrotasks, resizeObservers, mutationObservers } = runtime();
  const root = new Element("root");
  const fragment = new Element("fragment", 11);
  const scope = new B.Scope();
  const row = new Element("row");
  row.rect = { left: 0, top: 0, width: 100, height: 40 };
  B.within(scope, () => F.reflow(fragment, [0.2, 1]));
  fragment.append(row);
  assert.equal(resizeObservers.length, 0);
  root.append(fragment);
  flushMicrotasks();
  assert.equal(resizeObservers.length, 1);
  assert.ok(resizeObservers[0].targets.has(root));
  assert.ok(resizeObservers[0].targets.has(row));
  row.rect = { ...row.rect, top: 60 };
  mutationObservers[0].callback();
  const layer = row.bernLayers.layers.get("reflow");
  assert.equal(layer.y.value, -60);
  assert.equal(layer.y.target, 0);
  row.remove();
  mutationObservers[0].callback();
  assert.equal(resizeObservers[0].targets.has(row), false);
  scope.dispose();
  assert.equal(resizeObservers[0].disconnected, true);
  assert.equal(mutationObservers[0].disconnected, true);
  assert.equal(root.bernReflow, undefined);
  assert.equal(root.childNodes.length, 0);
});

test("reflow registrations share observers and remount after the last owner leaves", () => {
  const { B, F, Element, flushMicrotasks, resizeObservers } = runtime();
  const root = new Element("root");
  const owners = [new B.Scope(), new B.Scope()];
  for (const owner of owners) {
    const fragment = new Element("fragment", 11);
    B.within(owner, () => F.reflow(fragment, [0.2, 1]));
    root.append(fragment);
  }
  flushMicrotasks();
  assert.equal(resizeObservers.length, 1);
  assert.equal(root.bernReflow.users, 2);
  owners[0].dispose();
  assert.equal(root.bernReflow.users, 1);
  assert.equal(resizeObservers[0].disconnected, undefined);
  owners[1].dispose();
  assert.equal(resizeObservers[0].disconnected, true);
  const replacement = new B.Scope();
  B.within(replacement, () => F.reflow(root, [0.2, 1]));
  assert.equal(resizeObservers.length, 2);
  replacement.dispose();
  assert.equal(root.bernReflow, undefined);
});

test("disposing a fragment before mounting never starts reflow observers", () => {
  const { B, F, Element, flushMicrotasks, resizeObservers } = runtime();
  const fragment = new Element("fragment", 11);
  const scope = new B.Scope();
  B.within(scope, () => F.reflow(fragment, [0.2, 1]));
  scope.dispose();
  const root = new Element("root");
  root.append(fragment);
  flushMicrotasks();
  assert.equal(resizeObservers.length, 0);
  assert.equal(root.childNodes.length, 0);
});

test("position-only scrolling aligns the leading item without carousel padding", () => {
  const { B, F, Element, frames, resizeObservers } = runtime();
  const root = new Element("root");
  const position = B.cell(1);
  const scope = new B.Scope();
  B.within(scope, () => F.ScrollView(root, {}, { position }, (node) => {
    node.rect = { left: 0, top: 0, width: 600, height: 400 };
    node.scrollTop = 0;
    for (let index = 1; index <= 2; index++) {
      const row = new Element("row");
      row.setAttribute("data-bern-item", index);
      row.bernKey = index;
      row.getBoundingClientRect = () => ({ left: 0, top: (index - 1) * 200 - node.scrollTop, width: 120, height: 180 });
      node.append(row);
    }
  }));
  const scroll = root.childNodes[0];
  while (frames.length) frames.shift()(0);
  for (let repeat = 0; repeat < 8; repeat++) resizeObservers[0].callback();
  assert.equal(scroll.style.paddingBlock, undefined);
  assert.equal(scroll.lastScroll.top, 0);
  position.set(2);
  assert.equal(scroll.lastScroll.top, 200);
  assert.equal(scroll.lastScroll.behavior, "smooth");
  scope.dispose();
});

test("center snapping remains stable when its own padding contributes to intrinsic viewport size", () => {
  const { B, F, Element, resizeObservers } = runtime();
  const root = new Element("root");
  const scope = new B.Scope();
  B.within(scope, () => F.ScrollView(root, {}, { snap: "center" }, (node) => {
    // Model WebKit's intrinsic flex sizing: padding adds to this view's
    // measured size instead of subtracting from a fixed outer height.
    Object.defineProperty(node, "clientHeight", { get: () => 400 + 2 * parseFloat(node.style.paddingBlock ?? "0") });
    node.getBoundingClientRect = () => ({ left: 0, top: 0, width: 600, height: node.clientHeight });
    const row = new Element("row");
    row.setAttribute("data-bern-item", "first");
    row.rect = { left: 0, top: 0, width: 120, height: 200 };
    node.append(row);
  }));
  const scroll = root.childNodes[0];
  for (let repeat = 0; repeat < 20; repeat++) {
    resizeObservers[0].callback();
    assert.equal(scroll.style.paddingBlock, "100px");
    assert.equal(scroll.clientHeight, 600);
  }
  scope.dispose();
});

test("center snapping measures the fixed viewport and leaves an empty list unpadded", () => {
  const { B, F, Element, resizeObservers, mutationObservers } = runtime();
  const root = new Element("root");
  const scope = new B.Scope();
  B.within(scope, () => F.ScrollView(root, {}, { axis: "horizontal", snap: "center" }, (node) => {
    node.rect = { left: 0, top: 0, width: 600, height: 300 };
    const row = new Element("row");
    row.setAttribute("data-bern-item", "first");
    row.rect = { left: 0, top: 0, width: 200, height: 300 };
    node.append(row);
  }));
  const scroll = root.childNodes[0];
  resizeObservers[0].callback();
  assert.equal(scroll.style.paddingInline, "200px");
  scroll.children[0].remove();
  mutationObservers[0].callback();
  assert.equal(scroll.style.paddingInline, "0px");
  scope.dispose();
});


test("controlled seek keeps the drag preview across engine ticks and commits on release", () => {
  const { B, Element } = runtime();
  const host = new Element("div");
  const current = B.cell(0.25);
  const commits = [];
  const scope = new B.Scope();
  const field = { get: () => current.get(), peek: () => current.peek(), set: value => { commits.push(value); current.set(value); } };
  B.within(scope, () => B.Slider(host, {}, { id: "seek", label: () => "Seek", field, min: 0, max: 1, controlled: true, labelHidden: true }));
  const input = host.children[0].children[0];
  input.events.pointerdown[0]();
  input.value = 0.6;
  input.events.input[0]();
  current.set(0.26);
  assert.equal(Number(input.value), 0.6);
  assert.deepEqual(commits, []);
  input.events.change[0]();
  assert.deepEqual(commits, [0.6]);
  current.set(0.61);
  assert.equal(Number(input.value), 0.61);
  scope.dispose();
});

test("positioned captions preserve negative offsets", () => {
  const { B, F, Element } = runtime();
  const node = new Element("div");
  const offset = B.cell(-68);
  const scope = new B.Scope();
  B.within(scope, () => F.bindPosition(node, "left", () => offset.get(), null));
  assert.equal(node.style.left, "-68px");
  offset.set(24);
  assert.equal(node.style.left, "24px");
  scope.dispose();
});

test("idle timeout survives unrelated playback ticks, pauses and cleans up", () => {
  const { B, Element, timers } = runtime();
  const state = B.cell({ paused: false, position: 0 });
  const win = { element: new Element("div"), idle: B.cell(false) };
  const scope = new B.Scope();
  B.within(scope, () => B.hooks.idle(win, 3, () => !state.get().paused));
  const count = timers.length;
  state.set({ paused: false, position: 1 });
  assert.equal(timers.length, count, "playback ticks must not restart the timeout");
  timers.at(-1).callback();
  assert.equal(win.idle.get(), true);
  state.set({ paused: true, position: 1 });
  assert.equal(win.idle.get(), false);
  assert.ok(timers.at(-1).cancelled);
  state.set({ paused: false, position: 1 });
  assert.equal(timers.length, count + 1);
  scope.dispose();
  assert.ok(timers.at(-1).cancelled);
  assert.equal(win.element.events.pointermove.length, 0);
});

test("a disabled shortcut leaves its key to the one under it, and unmounting removes it", () => {
  const { B, F, Element, document } = runtime();
  const ran = [];
  const outer = new B.Scope();
  B.within(outer, () => F.KeyShortcut(null, {}, { key: "escape" }, () => ran.push("page")));
  const open = B.cell(true);
  const inner = new B.Scope();
  B.within(inner, () => F.KeyShortcut(null, {}, { key: "escape", enabled: () => open.get() }, () => ran.push("details")));
  const press = (key) => {
    const event = { key, code: key, target: new Element("div"), prevented: false, preventDefault() { this.prevented = true; } };
    for (const listener of document.listeners.keydown ?? []) listener(event);
    return event.prevented;
  };
  assert.equal(document.listeners.keydown.length, 1, "one listener for every shortcut");
  assert.equal(press("Escape"), true);
  assert.deepEqual(ran, ["details"], "the shortcut mounted last takes the key");
  open.set(false);
  press("Escape");
  assert.deepEqual(ran, ["details", "page"], "a disabled shortcut leaves the key to the one under it");
  inner.dispose();
  open.set(true);
  press("Escape");
  assert.deepEqual(ran, ["details", "page", "page"]);
  outer.dispose();
  assert.equal(press("Escape"), false, "no shortcut, no key taken");
});

test("chip pickers in a flow are its items and mark the chosen option", () => {
  const { B, Element } = runtime();
  const root = new Element("root");
  const field = B.cell("all");
  const options = [
    { name: "all", value: "all", label: () => "All" },
    { name: "tools", value: "tools", label: () => "Tools" },
  ];
  const scope = new B.Scope();
  B.within(scope, () => B.Flow(root, { comp: "demo" }, { gap: 10 }, (flow, ctx) => {
    B.Spacer(flow);
    B.Picker(flow, ctx, { id: "category", label: () => "Category", field, options, style: "chips", labelHidden: true });
  }));
  const flow = root.childNodes[0];
  const [spacer, chips] = flow.childNodes;
  assert.ok(spacer.classes.has("bern-spacer"));
  assert.ok(chips.classes.has("bern-chips"));
  assert.equal(chips.attributes.role, "radiogroup");
  assert.equal(chips.attributes["aria-label"], "Category");
  const [all, tools] = chips.childNodes;
  assert.ok(all.classes.has("bern-chip"));
  assert.equal(all.attributes.role, "radio");
  assert.equal(all.attributes["aria-checked"], "true");
  tools.events.click[0]();
  assert.equal(field.peek(), "tools");
  assert.equal(all.attributes["aria-checked"], "false");
  assert.equal(tools.attributes["aria-checked"], "true");
  const control = B.controlEntries().find((entry) => entry.target() === "demo.category");
  assert.equal(control.actions.value(), "tools");
  assert.equal(control.actions.select("all"), true);
  assert.equal(all.attributes["aria-checked"], "true");
  scope.dispose();
  const css = readFileSync(new URL("../templates/bern.css", import.meta.url), "utf8");
  // In a flow the group takes no box, so each chip wraps on its own; a
  // spacer takes the room left on its line, and items centre in it.
  assert.match(css, /\.bern-flow > \.bern-chips\s*\{\s*display:\s*contents;/);
  assert.match(css, /\.bern-flow\s*\{[^}]*flex-flow:\s*row wrap;[^}]*align-items:\s*center;/);
  assert.match(css, /\.bern-spacer\s*\{\s*flex:\s*1;/);
});
