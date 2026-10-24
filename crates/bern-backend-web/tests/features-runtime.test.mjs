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
      this.dataset = {};
      this.events = {};
      this.style = { setProperty(name, value) { this[name] = value; } };
      this.classes = new Set();
      this.classList = {
        add: (...names) => names.forEach((name) => this.classes.add(name)),
        remove: (...names) => names.forEach((name) => this.classes.delete(name)),
        toggle: (name, enabled) => enabled ? this.classes.add(name) : this.classes.delete(name),
        contains: (name) => this.classes.has(name),
      };
    }
    set className(value) { this.classes = new Set(value.split(/\s+/)); }
    get className() { return [...this.classes].join(" "); }
    setAttribute(name, value) { this.attributes[name] = String(value); }
    removeAttribute(name) { delete this.attributes[name]; }
    hasAttribute(name) { return name in this.attributes; }
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
    getClientRects() { return [this.getBoundingClientRect()]; }
    get nextSibling() {
      const siblings = this.parentNode?.childNodes ?? [];
      return siblings[siblings.indexOf(this) + 1] ?? null;
    }
    before(...nodes) {
      const parent = this.parentNode;
      const placed = nodes.flatMap((node) => node.nodeType === 11 ? node.childNodes.splice(0) : [node]);
      parent.childNodes.splice(parent.childNodes.indexOf(this), 0, ...placed);
      for (const node of placed) { node.parentNode = parent; node.parentElement = parent.nodeType === 1 ? parent : null; }
    }
    contains(other) {
      for (let node = other; node; node = node.parentNode) if (node === this) return true;
      return false;
    }
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
    replaceChildren(...children) {
      for (const child of [...this.childNodes]) child.remove();
      this.append(...children);
    }
    matches() { return false; }
    setPointerCapture(id) { this.captured = id; }
    focus() { this.focused = true; }
    /// Runs this element's listeners for `name`; returns whether one
    /// prevented the default.
    dispatch(name, init = {}) {
      let prevented = false;
      for (const callback of [...(this.events[name] ?? [])]) callback({ ...init, preventDefault() { prevented = true; } });
      return prevented;
    }
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
    TextEncoder, TextDecoder, URLSearchParams, performance, innerWidth: 1280, innerHeight: 800, HTMLElement: Element,
    navigator: { platform: "MacIntel" }, location: { search }, Image: Probe, screen: {},
    Node: { ELEMENT_NODE: 1, DOCUMENT_POSITION_FOLLOWING: 4 },
    document: {
      documentElement: new Element("html"),
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
    getComputedStyle: (node) => ({ colorScheme: node.style.colorScheme ?? "light", direction: node.style.direction ?? "ltr", paddingTop: node.style.paddingBlock ?? "0px", paddingLeft: node.style.paddingInline ?? "0px" }),
    requestAnimationFrame: (callback) => { frames.push(callback); return frames.length; },
    cancelAnimationFrame() {},
    setTimeout: (callback, delay) => { timers.push({ callback, delay }); return timers.length; },
    clearTimeout(id) { if (timers[id - 1]) timers[id - 1].cancelled = true; },
    queueMicrotask: (callback) => microtasks.push(callback),
    reportError(error) { throw error; },
  };
  const context = vm.createContext(globals);
  for (const [name, extra] of [["bern", ["chooseLanguage"]], ["features", ["transitionFrom"]]]) {
    const source = readFileSync(new URL(`../templates/${name}.js`, import.meta.url), "utf8");
    const exported = [...source.matchAll(/^export (?:async )?(?:class|function|const) (\w+)/gm)].map((match) => match[1]);
    const plain = source.replace(/^import .+;$/gm, "").replace(/^export /gm, "");
    vm.runInContext(`globalThis.${name === "bern" ? "B" : "F"} = (() => { ${plain}\nreturn { ${[...exported, ...extra].join(", ")} }; })();`, context, { filename: `${name}.js` });
  }
  const flushMicrotasks = () => { while (microtasks.length) microtasks.shift()(); };
  return { B: context.B, F: context.F, Element, document: globals.document, navigator: globals.navigator, screen: globals.screen, probes, queries, timers, frames, flushMicrotasks, resizeObservers, mutationObservers };
}

test("preview creates only rows fitting the viewport and releases rows when it shrinks", () => {
  const { B, Element, flushMicrotasks, resizeObservers } = runtime();
  const scope = new B.Scope();
  const root = new Element("div");
  const items = B.cell(Array.from({ length: 1000 }, (_, id) => ({ id })));
  const active = new Set(), created = [];
  B.within(scope, () => B.PreviewList(root, {}, {
    items: () => items.get(), key: item => item.id, itemHeight: 97, gap: 10, moreHeight: 22,
  }, (parent, ctx, item) => {
    const id = item().id;
    active.add(id); created.push(id);
    parent.append(new Element("img"));
    B.onCleanup(() => active.delete(id));
  }));
  const preview = root.children[0];
  const more = preview.children[1];
  const observer = resizeObservers.find(one => one.targets.has(preview));
  const resize = (height) => { observer.callback([{ contentRect: { height } }]); flushMicrotasks(); };
  assert.equal(active.size, 0, "unmeasured previews must not request images");
  resize(650);
  assert.deepEqual([...active], [0, 1, 2, 3, 4]);
  assert.equal(more.textContent, "+995");
  resize(900);
  assert.equal(active.size, 8);
  assert.equal(created.length, 8, "kept rows retain their identity");
  resize(300);
  assert.deepEqual([...active], [0, 1]);
  assert.equal(more.textContent, "+998");
  items.set(items.get().slice(0, 3)); flushMicrotasks();
  resize(311);
  assert.equal(active.size, 3);
  assert.equal(more.hidden, true, "no badge or reserved gap when everything fits");
  items.set([]); flushMicrotasks();
  assert.equal(active.size, 0);
  assert.equal(more.hidden, true);
  scope.dispose();
  assert.equal(observer.disconnected, true);
});

test("the interface language follows the browser's preferences in order", () => {
  const { B, navigator } = runtime();
  const choose = (languages, supported) => {
    Object.assign(navigator, { languages, language: languages[0] });
    return B.chooseLanguage(supported, "en");
  };
  assert.equal(choose(["de-DE", "en"], ["en", "de"]), "de", "an earlier language wins over a later exact match");
  assert.equal(choose(["zh-CN", "en"], ["en", "zh-Hans"]), "zh-Hans");
  assert.equal(choose(["zh-TW", "en"], ["en", "zh-Hans", "zh-Hant"]), "zh-Hant");
  assert.equal(choose(["pt-BR", "en"], ["pt", "pt-BR"]), "pt-BR", "an exact match still wins within one language");
  assert.equal(choose(["EN-us"], ["fr", "en-US"]), "en-US");
  assert.equal(choose(["ja"], ["de", "fr"]), "en");
});

test("transparent surfaces and plain controls shadow their content while filled controls keep box shadows", () => {
  const { B, F, Element } = runtime();
  for (const className of ["bern-layers", "bern-button-plain", "bern-icon-button", "bern-icon-button bern-icon-button-prominent", "bern-button"]) {
    const node = new Element("div");
    node.className = className;
    const scope = new B.Scope();
    B.within(scope, () => F.applyStyle(node, {
      base: { shadow: [3, 1, 0.9] },
      hover: { background: { role: "fill.subtle" } },
    }));
    const content = ["bern-layers", "bern-button-plain", "bern-icon-button"].includes(className);
    assert.equal(node.style.filter, content ? "drop-shadow(0 1px 3px rgb(0 0 0 / 0.9))" : "");
    assert.equal(node.style.boxShadow, content ? "" : "0 1px 3px rgb(0 0 0 / 0.9)");
    node.dispatch("pointerenter", { pointerType: "mouse" });
    assert.equal(node.style.filter, "");
    assert.equal(node.style.boxShadow, "0 1px 3px rgb(0 0 0 / 0.9)");
    node.dispatch("pointerleave");
    assert.equal(node.style.filter, content ? "drop-shadow(0 1px 3px rgb(0 0 0 / 0.9))" : "");
    scope.dispose();
  }
});

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

test("content capsules are the standard button and keep their children", () => {
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
  B.within(scope, () => F.ContentButton(root, { comp: "demo" }, {
    id: "plain", label: () => "Open", height: () => height.get(),
  }, () => {}, () => {}));
  const [button, plain] = root.childNodes;
  // The web has no capsule buttons: the standard one, at its own height.
  assert.ok(button.classes.has("bern-control-capsule"));
  assert.equal(button.style.borderRadius, undefined);
  assert.equal(button.style.height, undefined);
  assert.equal(button.childNodes[0], child);
  assert.equal(button.attributes["aria-label"], "Enable");
  // Any other content button takes the height it declares.
  assert.equal(plain.style.height, "30px");
  height.set(42);
  enabled.set(false);
  assert.equal(plain.style.height, "42px");
  assert.equal(button.disabled, true);
  const control = B.controlEntries().find((entry) => entry.target() === "demo.capsule");
  control.actions.activate();
  assert.equal(activations, 1);
  scope.dispose();
  assert.equal(B.controlEntries().length, 0);
});

test("an icon-only tab shows its icon alone, named by its label", () => {
  const { B, Element } = runtime();
  const root = new Element("root");
  const selection = B.cell("about");
  const label = B.cell("About");
  const scope = new B.Scope();
  B.within(scope, () => B.Tabs(root, {}, { selection }, (p, ctx) => {
    B.Tab(p, ctx, { value: "about", name: "about", label: () => label.get(), icon: "info.circle", iconOnly: true, build: () => {} });
    B.Tab(p, ctx, { value: "keys", name: "keys", label: () => "Shortcuts", icon: null, build: () => {} });
  }));
  const [about, keys] = root.childNodes[0].childNodes[0].children;
  assert.ok(about.classes.has("bern-tab-icon-only"));
  assert.deepEqual(about.childNodes.map((child) => child.tagName), ["svg"]);
  assert.equal(about.attributes["aria-label"], "About");
  assert.equal(about.attributes.title, "About");
  label.set("Info");
  assert.equal(about.attributes["aria-label"], "Info");
  assert.equal(about.attributes.title, "Info");
  assert.ok(!keys.classes.has("bern-tab-icon-only"));
  assert.equal(keys.attributes["aria-label"], undefined);
  assert.deepEqual(keys.childNodes.map((child) => child.textContent), ["Shortcuts"]);
  scope.dispose();
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
  // A declared width is exact and has nothing to drag.
  assert.deepEqual(root.childNodes[0].children.map((child) => child.className), ["bern-sidebar", "bern-detail"]);
  assert.doesNotMatch(css.match(/\n\.bern-sidebar \{[^}]*\}/)[0], /min-width|max-width/);
});

test("a sidebar of no declared width drags and steps between 200 and 320", () => {
  const { B, F, Element } = runtime();
  const root = new Element("root");
  F.SplitView(root, {}, { selection: B.cell(0), titled: () => false }, () => {}, () => {});
  const split = root.childNodes[0];
  const [sidebar, handle] = split.children;
  assert.equal(handle.className, "bern-split-handle");
  assert.equal(handle.attributes.role, "separator");
  assert.equal(handle.attributes.tabindex, "0");
  assert.equal(handle.attributes["aria-orientation"], "vertical");
  assert.equal(handle.attributes["aria-label"], "bern.navigation.sidebar");
  assert.equal(handle.attributes["aria-controls"], sidebar.attributes.id);
  assert.deepEqual([handle.attributes["aria-valuemin"], handle.attributes["aria-valuenow"], handle.attributes["aria-valuemax"]], ["200", "232", "320"]);
  assert.equal(sidebar.style["--bern-sidebar-width"], "232px");

  // A pointer drags it from where it was pressed, held to the range.
  assert.equal(handle.dispatch("pointerdown", { button: 2, clientX: 232, pointerId: 1 }), false, "only the main button drags");
  assert.equal(handle.dispatch("pointerdown", { button: 0, clientX: 232, pointerId: 7 }), true);
  assert.equal(handle.captured, 7);
  assert.equal(split.classes.has("bern-split-resizing"), true);
  handle.dispatch("pointermove", { clientX: 290.4 });
  assert.equal(sidebar.style["--bern-sidebar-width"], "290px");
  assert.equal(handle.attributes["aria-valuenow"], "290");
  handle.dispatch("pointermove", { clientX: 900 });
  assert.equal(sidebar.style["--bern-sidebar-width"], "320px");
  handle.dispatch("pointermove", { clientX: -50 });
  assert.equal(sidebar.style["--bern-sidebar-width"], "200px");
  handle.dispatch("pointermove", { clientX: 270 });
  handle.dispatch("lostpointercapture");
  assert.equal(split.classes.has("bern-split-resizing"), false);
  handle.dispatch("pointermove", { clientX: 300 });
  assert.equal(sidebar.style["--bern-sidebar-width"], "270px", "released, it no longer follows the pointer");

  // The arrow keys step it by 10, Home and End take it to either end.
  const key = (name) => handle.dispatch("keydown", { key: name });
  assert.equal(key("ArrowRight"), true);
  assert.equal(handle.attributes["aria-valuenow"], "280");
  key("ArrowLeft");
  key("ArrowLeft");
  assert.equal(handle.attributes["aria-valuenow"], "260");
  key("End");
  key("ArrowRight");
  assert.equal(handle.attributes["aria-valuenow"], "320");
  key("Home");
  key("ArrowLeft");
  assert.equal(handle.attributes["aria-valuenow"], "200");
  assert.equal(key("Tab"), false, "other keys pass through");

  // Right to left, the sidebar is on the right and grows to the left.
  split.style.direction = "rtl";
  key("ArrowLeft");
  assert.equal(handle.attributes["aria-valuenow"], "210");
  handle.dispatch("pointerdown", { button: 0, clientX: 500, pointerId: 2 });
  handle.dispatch("pointermove", { clientX: 450 });
  assert.equal(sidebar.style["--bern-sidebar-width"], "260px");

  const css = readFileSync(new URL("../templates/bern.css", import.meta.url), "utf8");
  assert.match(css, /\.bern-split-handle \{[^}]*cursor:\s*col-resize/);
  assert.match(css, /\.bern-split-handle:focus-visible::after \{[^}]*background:\s*var\(--bern-focus-indicator\)/);
});

test("the sidebar button hides the whole sidebar and shows it again from the page header", () => {
  for (const [search, fold] of [["", "0.25s"], ["?motion=reduce", "0s"]]) {
    const { B, F, Element } = runtime(search);
    const root = new Element("root");
    F.SplitView(root, {}, { selection: B.cell(0), titled: () => false }, () => {}, () => {});
    const split = root.childNodes[0];
    const [sidebar, , detail] = split.children;
    const header = detail.children[0];
    const [back, toggle] = header.children;
    assert.equal(back.className, "bern-icon-button bern-split-back");
    assert.equal(toggle.className, "bern-icon-button bern-split-toggle");
    assert.equal(toggle.attributes["aria-controls"], sidebar.attributes.id);
    assert.equal(toggle.attributes["aria-label"], "bern.navigation.hide_sidebar");
    assert.equal(toggle.attributes.title, "bern.navigation.hide_sidebar");
    toggle.dispatch("click");
    assert.equal(split.classes.has("bern-split-collapsed"), true);
    assert.equal(split.style["--bern-sidebar-fold"], fold, "reduced motion hides it at once");
    assert.equal(toggle.attributes["aria-label"], "bern.navigation.show_sidebar");
    assert.equal(header.children[1], toggle, "the button keeps its place, and focus with it");
    toggle.dispatch("click");
    assert.equal(split.classes.has("bern-split-collapsed"), false);
    assert.equal(toggle.attributes.title, "bern.navigation.hide_sidebar");
  }
  const css = readFileSync(new URL("../templates/bern.css", import.meta.url), "utf8");
  const wide = css.match(/@container \(min-width: 768px\) \{[\s\S]*?\n\}/)[0];
  assert.match(wide, /\.bern-split-collapsed > \.bern-sidebar \{[^}]*margin-inline-start:\s*calc\(-1 \* var\(--bern-sidebar-width[^}]*visibility:\s*hidden/);
  assert.match(wide, /\.bern-split-collapsed > \.bern-split-handle \{\s*display:\s*none/);
  // On narrow windows the sidebar is a page of its own: no button, no splitter.
  assert.match(css, /@container \(max-width: 767px\) \{[\s\S]*?\.bern-split-handle,\s*\.bern-icon-button\.bern-split-toggle \{\s*display:\s*none/);
});

test("tabs under conditions come and go in their written order, and a selection without its tab moves on", () => {
  const { B, Element, flushMicrotasks } = runtime();
  const root = new Element("root");
  const anime = B.cell(true);
  const library = B.cell(false);
  const selection = B.cell("home");
  const built = [];
  const tab = (value) => (p, ctx) => B.Tab(p, ctx, {
    value, name: value, label: () => value, icon: null,
    build: (p) => { built.push(value); p.append(new Element(value)); },
  });
  B.Tabs(root, { comp: "shell" }, { id: "tabs", selection, fallback: "new" }, (p, ctx) => {
    B.When(p, ctx, () => (anime.get() ? 0 : -1), [(p, ctx) => { tab("home")(p, ctx); tab("new")(p, ctx); }]);
    B.When(p, ctx, () => (library.get() ? 0 : -1), [tab("library")]);
    tab("search")(p, ctx);
  });
  const [bar, panels] = root.childNodes[0].children;
  const names = () => bar.children.map((button) => button.children.at(-1).textContent);
  const page = () => panels.children.map((child) => child.tagName);
  const selected = () => names().filter((_, index) => bar.children[index].attributes["aria-selected"] === "true");
  const control = () => B.controlEntries().find((entry) => entry.role === "tabs");
  flushMicrotasks();
  assert.deepEqual(names(), ["home", "new", "search"]);
  assert.deepEqual(selected(), ["home"]);
  assert.deepEqual(page(), ["home"]);
  assert.equal(control().label(), "home");

  // A tab whose condition turns on takes its written place, before `search`.
  library.set(true);
  flushMicrotasks();
  assert.deepEqual(names(), ["home", "new", "library", "search"]);
  bar.children[2].dispatch("click");
  assert.equal(selection.peek(), "library");
  assert.deepEqual(page(), ["library"]);
  assert.equal(control().actions.value(), "library");

  // The arrow keys move along the tabs shown, wrapping at the ends.
  bar.dispatch("keydown", { key: "ArrowRight", target: bar.children[2] });
  assert.equal(selection.peek(), "search");
  assert.equal(bar.children[3].focused, true);
  bar.dispatch("keydown", { key: "ArrowRight", target: bar.children[3] });
  assert.equal(selection.peek(), "home");
  assert.equal(control().actions.select("library"), true);

  // The selected tab goes away: the fallback, while it is shown.
  library.set(false);
  flushMicrotasks();
  assert.deepEqual(names(), ["home", "new", "search"]);
  assert.equal(selection.peek(), "new");
  assert.deepEqual(page(), ["new"]);
  assert.equal(control().actions.select("library"), false, "a tab that is not mounted cannot be chosen");

  // With the fallback gone too, the first tab left.
  anime.set(false);
  flushMicrotasks();
  assert.deepEqual(names(), ["search"]);
  assert.equal(selection.peek(), "search");
  assert.deepEqual(page(), ["search"]);
  assert.deepEqual(built, ["home", "library", "search", "home", "library", "new", "search"]);
});

test("a stack holding a form or a scroll view fills a page's room as it fills a column", () => {
  // A navigation page, a tab's page and the layout an `Adaptive` chose are
  // the page's room: a flexible child stretches across and grows down it.
  const css = readFileSync(new URL("../templates/features.css", import.meta.url), "utf8");
  const rule = (body) => css.split("}").map((block) => block.split("{")).filter(([selector, rest]) => rest?.includes(body)).map(([selector]) => selector);
  const across = rule("align-self: stretch;\n  width: auto;\n").find((selector) => selector.includes(":is(.bern-fill-width, .bern-layout-width)"));
  const down = rule("flex: 1 1 auto;\n  min-height: 0;").find((selector) => selector.includes(":is(.bern-fill-height, .bern-layout-height)") && selector.includes(".bern-detail-body"));
  for (const host of [".bern-detail-body", ".bern-nav-page", ".bern-tabpanels", ".bern-adaptive"]) {
    assert.ok(across.includes(host), `${host} stretches a flexible child across`);
    assert.ok(down.includes(host), `${host} grows a flexible child down`);
  }
});

test("a search field without a width is as wide as its column, and a toolbar's keeps its width", () => {
  const { B, Element } = runtime();
  const root = new Element("root");
  const scope = new B.Scope();
  // Results whose list stretches across and down their popover.
  const results = (panel) => {
    const list = new Element("div");
    list.classList.add("bern-fill-width", "bern-fill-height");
    panel.append(list);
  };
  B.within(scope, () => B.Stack(root, { comp: "demo" }, { axis: "column" }, (column, ctx) => {
    B.Search(column, ctx, { id: "plain", field: B.cell(""), prompt: () => "Search", height: 34 }, null, results);
    B.Search(column, ctx, { id: "sized", field: B.cell(""), width: 240 }, null, results);
  }));
  B.within(scope, () => B.Search(root, { comp: "demo", inToolbar: true }, { id: "tool", field: B.cell("") }));
  const [column, tool] = root.childNodes;
  const [plain, sized] = column.childNodes;
  // In a page: as wide as its column, and in a row the room left.
  assert.ok(plain.classes.has("bern-fill-width"));
  assert.ok(plain.classes.has("bern-layout-width"));
  assert.equal(plain.style.width, undefined);
  assert.equal(plain.style.height, "34px");
  // A width of its own stays, however its results stretch.
  assert.ok(sized.classes.has("bern-sized-width"));
  assert.ok(!sized.classes.has("bern-fill-width"));
  assert.ok(!sized.classes.has("bern-layout-width"));
  assert.equal(sized.style.width, "240px");
  // Neither grows down its column.
  for (const box of [plain, sized]) {
    assert.ok(box.classes.has("bern-sized-height"));
    assert.ok(!box.classes.has("bern-layout-height"));
  }
  // In a toolbar: the toolbar's width.
  assert.ok(!tool.classes.has("bern-fill-width"));
  assert.ok(!tool.classes.has("bern-sized-width"));
  scope.dispose();
  const css = readFileSync(new URL("../templates/bern.css", import.meta.url), "utf8");
  assert.doesNotMatch(css.match(/\n\.bern-search \{[^}]*\}/)[0], /width/, "the box takes its width from its place");
});

test("a scroll view in a split button's or popover button's panel leaves the button its size", () => {
  const { B, F, Element, mutationObservers } = runtime();
  const root = new Element("root");
  const scope = new B.Scope();
  // A scroll view across and down its panel.
  const scrolled = (panel) => {
    const view = new Element("div");
    view.classList.add("bern-scroll", "bern-fill-width", "bern-fill-height");
    panel.append(view);
  };
  B.within(scope, () => B.Stack(root, { comp: "demo" }, { axis: "row" }, (row, ctx) => {
    F.SplitButton(row, ctx, { id: "caption", label: () => "Caption", field: B.cell(true) }, scrolled);
    F.PopoverButton(row, ctx, { id: "listed", label: () => "Listed" }, scrolled);
  }));
  const [row] = root.children;
  const [split, wrap] = row.children;
  const panelOf = (node) => node.children.find((child) => child.hasAttribute("popover"));
  const flexible = (node) => ["width", "height"].filter((dimension) => node.classes.has(`bern-layout-${dimension}`));
  // The split button's panel holds its content from the start.
  assert.equal(panelOf(split).children.length, 1);
  assert.deepEqual(flexible(split), []);
  assert.deepEqual(flexible(row), []);
  // The popover button's content mounts as its panel opens, and the row
  // looks at its children again.
  const panel = panelOf(wrap);
  panel.dispatch("beforetoggle", { newState: "open" });
  assert.equal(panel.children.length, 1);
  mutationObservers.find((observer) => observer.targets.has(row)).callback([{ addedNodes: panel.children, removedNodes: [] }]);
  assert.deepEqual(flexible(wrap), []);
  assert.deepEqual(flexible(row), []);
  // In its panel the scroll view still fills the panel.
  for (const holder of [panelOf(split), panel]) assert.deepEqual(flexible(holder.children[0]), ["width", "height"]);
  scope.dispose();
});

test("popup surface follows the window theme without recoloring its dark trigger", () => {
  const { B, F, Element, document } = runtime();
  const player = new Element("player"), scope = new B.Scope();
  player.classList.add("bern-appearance-dark");
  B.within(scope, () => F.PopoverButton(player, { comp: "player" }, {
    id: "rate", label: () => "Rate", windowTheme: true,
    style: { base: { background: { light: "#F2F2F2", dark: "#1E1E1E" }, foreground: { role: "text.primary" } } },
  }, () => {}));
  const [trigger, panel] = player.children[0].children;
  assert.equal(panel.classList.contains("bern-appearance-light"), true);
  assert.equal(panel.style.background, "light-dark(#F2F2F2, #1E1E1E)");
  assert.equal(panel.style.color, "var(--bern-text-primary)");
  assert.equal(trigger.style.background, undefined);
  document.documentElement.style.colorScheme = "dark";
  F.themeTick.set(F.themeTick.peek() + 1);
  assert.equal(panel.classList.contains("bern-appearance-dark"), true);
  assert.equal(panel.classList.contains("bern-appearance-light"), false);
  document.documentElement.style.colorScheme = "light";
  F.themeTick.set(F.themeTick.peek() + 1);
  assert.equal(panel.classList.contains("bern-appearance-light"), true);
  assert.equal(player.classList.contains("bern-appearance-dark"), true);
  scope.dispose();
});

test("controlled popovers publish native changes before deferred toggle and release every content scope", () => {
  const { B, F, Element, flushMicrotasks } = runtime();
  const root = new Element("root"), scope = new B.Scope();
  const state = B.cell({ open: false, tick: 0 });
  const writes = [];
  let mounted = 0, released = 0;
  B.within(scope, () => F.PopoverButton(root, { comp: "demo" }, {
    id: "settings", label: () => "Settings",
    presented: {
      get: () => state.get().open, peek: () => state.peek().open,
      set: (open) => { writes.push(open); state.set({ ...state.peek(), open }); },
    },
  }, (panel) => {
    mounted++;
    B.onCleanup(() => released++);
    panel.append(new Element("content"));
  }));
  const [trigger, panel] = root.children[0].children;
  panel.isConnected = true;
  let showing = false;
  let transitioning = false;
  panel.matches = () => showing;
  const nativeChange = (next) => {
    if (showing === next) return;
    assert.equal(transitioning, false, "native popovers reject reentrant show operations");
    transitioning = true;
    panel.dispatch("beforetoggle", { newState: next ? "open" : "closed" });
    // Engine microtasks may flush before the browser completes its show/hide.
    state.set({ ...state.peek() });
    showing = next;
    transitioning = false;
  };
  panel.showPopover = () => nativeChange(true);
  panel.hidePopover = () => nativeChange(false);
  flushMicrotasks();
  panel.showPopover();
  assert.equal(state.peek().open, true, "opening publishes without waiting for toggle");
  for (let tick = 1; tick <= 3; tick++) {
    panel.hidePopover();
    assert.equal(state.peek().open, false, "native dismissal publishes synchronously");
    assert.equal(panel.children.length, 0);
    state.set({ ...state.peek(), tick });
    assert.equal(showing, false, "a player tick must not reopen dismissed settings");
    state.set({ ...state.peek(), open: true });
    assert.equal(panel.children.length, 1);
    assert.equal(mounted - released, 1);
    // Browsers may coalesce close/reopen into one delayed toggle event.
    panel.dispatch("toggle", { oldState: "open", newState: "open" });
    assert.equal(panel.children.length, 1);
  }
  assert.equal(trigger.attributes["aria-expanded"], "true");
  assert.deepEqual(writes, [true, false, false, false]);
  scope.dispose();
  assert.equal(mounted, released);
  assert.equal(state.peek().open, true, "unmounting must not overwrite the next layout's binding");
  flushMicrotasks();
  assert.equal(showing, false);
});

test("engine popovers keep the latest native intent until its command completes", () => {
  const { B, F, Element, flushMicrotasks } = runtime();
  const root = new Element("root"), scope = new B.Scope();
  const state = B.cell({ open: false, tick: 0 });
  const requests = [];
  const commit = (open, done) => requests.push({ open, done });
  let mounted = 0, released = 0;
  B.within(scope, () => F.PopoverButton(root, { comp: "demo" }, {
    id: "settings", label: () => "Settings",
    presented: { get: () => state.get().open, peek: () => state.peek().open, set: commit, commit },
  }, panel => {
    mounted++;
    B.onCleanup(() => released++);
    panel.append(new Element("content"));
  }));
  const [trigger, panel] = root.children[0].children;
  let showing = false;
  panel.isConnected = true;
  panel.matches = () => showing;
  const change = next => {
    if (showing === next) return;
    panel.dispatch("beforetoggle", { newState: next ? "open" : "closed" });
    showing = next;
  };
  panel.showPopover = () => change(true);
  panel.hidePopover = () => change(false);
  const tick = () => state.set({ ...state.peek(), tick: state.peek().tick + 1 });
  const ack = (index, error = null) => {
    const request = requests[index];
    if (!error) state.set({ ...state.peek(), open: request.open });
    request.done?.(error);
    flushMicrotasks();
  };
  flushMicrotasks();
  panel.showPopover();
  tick();
  assert.equal(showing, true, "stale player frames cannot close newly opened settings");
  assert.equal(mounted, 1, "no duplicate mount or entrance animation during acknowledgement");
  panel.hidePopover();
  assert.deepEqual(requests.map(r => r.open), [true, false], "closing must supersede an in-flight open even when the record still says closed");
  ack(0);
  tick();
  assert.equal(showing, false, "an earlier acknowledgement cannot reopen dismissed settings");
  ack(1);
  assert.equal(showing, false);
  panel.showPopover();
  ack(2);
  tick();
  assert.equal(showing, true);
  panel.hidePopover();
  ack(3, new Error("rejected"));
  assert.equal(showing, true, "failed commits reconcile with authoritative state");
  state.set({ ...state.peek(), open: false });
  assert.equal(showing, false, "external commands still control the popover after completion");
  assert.equal(trigger.attributes["aria-expanded"], "false");
  panel.showPopover();
  scope.dispose();
  ack(4);
  assert.equal(showing, false, "late completion cannot reopen an unmounted panel");
  assert.equal(mounted, released);
});

test("popover entrance animations are cancelled on closing and replaced on reopening", () => {
  const { Element, document } = runtime();
  const panel = new Element("div"), animations = [];
  let open = false;
  panel.matches = selector => selector === ":popover-open" ? open : true;
  panel.animate = (_keyframes, options) => {
    assert.ok(panel.classes.has("bern-popover-entering"), "the first-paint guard stays until animation creation");
    assert.equal(options.fill, "backwards", "the animation start applies while its timeline is pending");
    const animation = { cancelled: false, cancel() { this.cancelled = true; } };
    animations.push(animation);
    return animation;
  };
  const toggle = (type, state) => {
    for (const listener of document.listeners[type] ?? []) listener({ target: panel, newState: state });
  };
  toggle("beforetoggle", "open");
  assert.ok(panel.classes.has("bern-popover-entering"), "arm the invisible first frame before the browser shows the panel");
  assert.equal(animations.length, 0, "position and content are measured before animating");
  open = true;
  toggle("toggle", "open");
  assert.equal(animations.length, 2);
  assert.equal(panel.classes.has("bern-popover-entering"), false);
  toggle("beforetoggle", "closed");
  open = false;
  assert.ok(animations.every(a => a.cancelled), "a closed popup must not retain scale/fade animations");
  assert.equal(panel.classes.has("bern-popover-entering"), false);
  toggle("toggle", "open");
  assert.equal(animations.length, 2, "stale deferred open events must not animate a hidden panel");
  toggle("beforetoggle", "open");
  open = true;
  toggle("toggle", "open");
  toggle("toggle", "open");
  assert.equal(animations.length, 4, "duplicate deferred toggles cannot restart the entrance");
  assert.ok(animations.slice(0, -2).every(a => a.cancelled));
});

for (const kind of ["popover", "split", "menu", "picker"]) {
test(`${kind} outside dismissal consumes a complete press and leaves the next press and inside controls usable`, () => {
  const { B, F, Element, document } = runtime();
  const root = new Element("root"), scope = new B.Scope();
  B.within(scope, () => {
    const ctx = { comp: "demo" }, spec = { id: "settings", label: () => "Settings" };
    if (kind === "popover") F.PopoverButton(root, ctx, spec, () => {});
    if (kind === "split") F.SplitButton(root, ctx, { ...spec, field: B.cell(true) }, () => {});
    if (kind === "menu") B.Menu(root, ctx, spec, []);
    if (kind === "picker") B.Picker(root, ctx, { ...spec, field: B.cell(1), options: [
      { value: 1, name: "one", label: () => "One" },
    ], style: "menu", labelHidden: true });
  });
  const children = root.children[0].children;
  const trigger = children.at(-2), panel = children.at(-1);
  const otherTrigger = new Element("button"), otherPanel = new Element("div");
  const otherIcon = new Element("svg");
  otherTrigger.append(otherIcon);
  B.within(scope, () => B.dismissPopoverOnOutsidePress(otherPanel, otherTrigger));
  let showing = true, closed = 0;
  panel.matches = () => showing;
  panel.hidePopover = () => { showing = false; closed++; };
  const outside = new Element("video"), inside = new Element("slider");
  panel.append(inside);
  const press = (type, target, pointerId = 7) => {
    let prevented = false, stopped = false;
    for (const listener of document.listeners[type] ?? []) {
      listener({ target, button: 0, pointerId, isPrimary: true,
        preventDefault() { prevented = true; }, stopImmediatePropagation() { stopped = true; },
      });
      if (stopped) break;
    }
    assert.equal(prevented, stopped);
    return prevented;
  };
  assert.equal(press("pointerdown", inside), false);
  assert.equal(press("pointerdown", trigger), false);
  assert.equal(closed, 0);
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {
    assert.equal(press(type, outside, type === "click" ? 1 : 7), true, type);
  }
  assert.equal(closed, 1);
  for (const type of ["pointerdown", "pointerup", "click"]) assert.equal(press(type, outside), false, type);
  showing = true;
  assert.equal(press("pointerdown", outside), true);
  assert.equal(press("pointercancel", outside), true);
  assert.equal(press("click", outside), false);
  showing = true;
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {
    assert.equal(press(type, otherIcon), false, "switching popup triggers must reach the new trigger");
  }
  assert.equal(showing, false, "switching closes the old popover");
  scope.dispose();
  showing = true;
  assert.equal(press("pointerdown", outside), false, "disposed panels are unregistered");
});
}

test("focus regions dismiss editing with one complete press and preserve inside controls", () => {
  const { B, Element, document } = runtime();
  const root = new Element("root"), scope = new B.Scope();
  const input = new Element("input"), send = new Element("button"), outside = new Element("video");
  input.value = "unsent draft";
  input.matches = () => true;
  input.blur = () => { document.activeElement = null; };
  B.within(scope, () => B.FocusRegion(root, {}, p => p.append(input, send)));
  const press = (type, target, detail = 1) => {
    let consumed = false;
    for (const listener of document.listeners[type] ?? []) {
      listener({ type, target, button: 0, pointerId: type === "click" ? 1 : 7, isPrimary: true, detail,
        preventDefault() {}, stopImmediatePropagation() { consumed = true; } });
      if (consumed) break;
    }
    return consumed;
  };
  document.activeElement = input;
  assert.equal(press("pointerdown", input), false);
  assert.equal(press("pointerdown", send), false);
  assert.equal(document.activeElement, input);
  assert.equal(press("pointerdown", outside), true);
  assert.equal(document.activeElement, null);
  assert.equal(input.value, "unsent draft");
  assert.equal(press("click", send, 0), false, "keyboard activation remains independent");
  for (const type of ["mousedown", "pointerup", "mouseup", "click"]) assert.equal(press(type, outside), true, type);
  assert.equal(press("pointerdown", outside), false, "next press works");
  document.activeElement = input;
  assert.equal(press("pointerdown", outside), true);
  assert.equal(press("pointercancel", outside), true);
  assert.equal(press("click", outside), false);
  scope.dispose();
  document.activeElement = input;
  assert.equal(press("pointerdown", outside), false, "unmounted regions unregister");
});

test("a toolbar appearing under a stationary mouse keeps the window awake", () => {
  const { B, Element, document, timers } = runtime();
  const root = new Element("div");
  const picture = new Element("button");
  const toolbar = new Element("button");
  picture.dataset = { idleAfter: "1" };
  picture.closest = selector => selector === ".bern-idle-area" ? picture : null;
  root.querySelector = selector => selector === ".bern-idle-area" ? picture : null;
  let hit = picture;
  document.elementFromPoint = () => hit;
  const win = { element: root, idle: B.cell(true) };
  const scope = new B.Scope();
  B.within(scope, () => B.hooks.idle(win, 3));
  const move = () => root.dispatch("pointermove", {
    target: picture, pointerType: "mouse", clientX: 110, clientY: 40,
  });
  move();
  assert.equal(win.idle.peek(), false);
  assert.equal(timers.at(-1).delay, 1000);
  // The entrance finishes without another mouse move. The original event
  // hit the picture, but the button now occupies that same screen position.
  hit = toolbar;
  timers.at(-1).callback();
  assert.equal(win.idle.peek(), false, "the appearing toolbar must not hide before it can be clicked");
  // Moving back onto the picture still hides the controls normally.
  hit = picture;
  move();
  timers.at(-1).callback();
  assert.equal(win.idle.peek(), true);
  // Leaving the window while a hide is pending also cancels that hide.
  move();
  root.dispatch("pointerleave", { pointerType: "mouse" });
  assert.equal(win.idle.peek(), false);
  assert.equal(timers.at(-1).cancelled, true);
  scope.dispose();
});

test("a window held in landscape locks the screen while it holds", () => {
  const { B, document, screen } = runtime();
  const calls = [];
  screen.orientation = {
    // Like a phone's browser: locking works only in fullscreen.
    lock(kind) {
      calls.push(`lock ${kind}`);
      return document.fullscreenElement ? Promise.resolve() : Promise.reject(new Error("NotSupportedError"));
    },
    unlock() { calls.push("unlock"); },
  };
  document.removeEventListener = function (name, callback) {
    this.listeners[name] = (this.listeners[name] ?? []).filter((one) => one !== callback);
  };
  const wanted = B.cell(false);
  const scope = new B.Scope();
  B.within(scope, () => B.hooks.landscape(() => wanted.get()));
  assert.deepEqual(calls, [], "a window that does not hold landscape leaves the orientation alone");
  wanted.set(true);
  assert.deepEqual(calls, ["lock landscape"]);
  // Refused outside fullscreen, the lock is asked for again once it begins.
  document.fullscreenElement = {};
  for (const callback of document.listeners.fullscreenchange) callback();
  assert.deepEqual(calls, ["lock landscape", "lock landscape"]);
  wanted.set(false);
  assert.equal(calls.at(-1), "unlock");
  for (const callback of document.listeners.fullscreenchange) callback();
  assert.equal(calls.at(-1), "unlock", "fullscreen does not lock a window that let go");
  wanted.set(true);
  scope.dispose();
  assert.equal(calls.at(-1), "unlock", "a closed window gives the orientation back");
  assert.equal(document.listeners.fullscreenchange.length, 0);
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

test("Escape belongs to an open native popover before the underlying page shortcut", () => {
  const { B, F, Element, document } = runtime();
  const scope = new B.Scope(), target = new Element("button");
  let ran = 0, prevented = false, open = true;
  document.querySelector = selector => selector === '[popover="auto"]:popover-open' && open ? target : null;
  B.within(scope, () => F.KeyShortcut(null, {}, { key: "escape" }, () => ran++));
  const press = () => {
    for (const listener of document.listeners.keydown) listener({ key: "Escape", target,
      preventDefault() { prevented = true; },
    });
  };
  press();
  assert.equal(ran, 0);
  assert.equal(prevented, false, "native dismissal must still receive Escape");
  open = false;
  press();
  assert.equal(ran, 1);
  assert.equal(prevented, true);
  scope.dispose();
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

test("a layout chosen by width is built for its window's width before it measures itself", () => {
  const { B, Element, resizeObservers } = runtime();
  const breakpoints = { compact: 600, medium: null, expanded: null };
  const built = [];
  const layouts = {
    compact: (p) => { built.push("compact"); p.append(new Element("div")); },
    expanded: (p) => { built.push("expanded"); p.append(new Element("div")); },
  };
  const mount = (width) => {
    const root = new Element("root");
    const scope = new B.Scope();
    B.within(scope, () => B.Adaptive(root, { win: { width }, comp: "demo" }, { id: "layout", breakpoints, layouts }));
    return { node: root.childNodes[0], scope, observer: resizeObservers.at(-1) };
  };
  const size = () => B.controlEntries().find((entry) => entry.target() === "demo.layout").actions.value();
  // A phone's window: only the phone's layout is ever built.
  const phone = mount(390);
  assert.deepEqual(built, ["compact"]);
  assert.equal(size(), "compact");
  phone.node.rect = { left: 0, top: 0, width: 390, height: 300 };
  phone.observer.callback([]);
  assert.deepEqual(built, ["compact"]);
  phone.scope.dispose();
  // A wide window never builds the narrow layout first; measured narrower
  // than the window, as beside a sidebar, the layout follows its room.
  built.length = 0;
  const wide = mount(1000);
  assert.deepEqual(built, ["expanded"]);
  assert.equal(size(), "expanded");
  wide.node.rect = { left: 0, top: 0, width: 500, height: 300 };
  wide.observer.callback([]);
  assert.deepEqual(built, ["expanded", "compact"]);
  assert.equal(size(), "compact");
  wide.scope.dispose();
});

test("a window is sized for the room it gets before it is laid out", () => {
  const { B } = runtime();
  // Arrays from the runtime's realm, compared as plain ones.
  const room = (...args) => Array.from(B.windowRoom(...args));
  const state = { frame: null, maximized: false };
  const phone = { width: 390, height: 844 };
  const desktop = { width: 1440, height: 900 };
  const page = { presentation: "page", width: 1100, height: 720 };
  assert.deepEqual(room(page, state, phone, true), [390, 844]);
  assert.deepEqual(room(page, state, desktop, false), [1440, 900]);
  const dialog = { presentation: "dialog", width: 480, height: 360 };
  assert.deepEqual(room(dialog, state, desktop, false), [480, 360]);
  assert.deepEqual(room({ presentation: "dialog", width: null, height: null }, state, desktop, false), [560, 900]);
  assert.deepEqual(room(dialog, state, phone, true), [390, 844], "a narrow desktop's dialog fills it");
  const floating = { presentation: "floating", width: 900, height: 600 };
  assert.deepEqual(room(floating, state, { width: 800, height: 700 }, false), [800, 600]);
  assert.deepEqual(room(floating, { frame: { width: 700, height: 500 }, maximized: false }, desktop, false), [700, 500]);
  assert.deepEqual(room(floating, { frame: { width: 700, height: 500 }, maximized: true }, desktop, false), [1440, 900]);
});

test("a popover panel is as high as its content and opens on the side that holds it", () => {
  const { B, Element } = runtime();
  // In a window 800 high, a trigger 36 high whose top is at `top`.
  const anchor = (top) => Object.assign(new Element("button"), { rect: { left: 40, right: 120, top, bottom: top + 36, width: 80, height: 36 } });
  const panel = () => {
    const node = new Element("div");
    node.classList.add("bern-popover-panel");
    // Shown with all its content it is 347 high; hidden it has no box.
    Object.defineProperty(node, "offsetHeight", { get: () => (node.style.display === "flex" ? 347 : 0) });
    return node;
  };
  // 152 below the trigger, 588 above: upwards, as high as the room.
  const low = panel();
  B.placePopover(low, anchor(600));
  assert.equal(low.style.top, "auto");
  assert.equal(low.style.bottom, "204px");
  assert.equal(low.style.maxHeight, "588px", "a panel has no list's 328 cap, so content that fits does not scroll");
  assert.equal(low.style.display, undefined, "measuring leaves the panel hidden");
  assert.equal(low.style.visibility, undefined);
  // 290 below, 450 above: only the room above holds the panel.
  const middle = panel();
  B.placePopover(middle, anchor(462));
  assert.equal(middle.style.top, "auto");
  assert.equal(middle.style.maxHeight, "450px");
  // 652 below: downwards.
  const high = panel();
  B.placePopover(high, anchor(100));
  assert.equal(high.style.top, "140px");
  assert.equal(high.style.bottom, "auto");
  assert.equal(high.style.maxHeight, "652px");
  // A list of options is at most 328 high and opens upwards only when
  // less than 160 is left below.
  const list = new Element("div");
  B.placePopover(list, anchor(600));
  assert.equal(list.style.top, "auto");
  assert.equal(list.style.maxHeight, "328px");
  B.placePopover(list, anchor(462));
  assert.equal(list.style.top, "502px");
  assert.equal(list.style.maxHeight, "290px");
});

test("scroll views scroll along their axis only and a segmented control narrows to its room", () => {
  const { B, Element } = runtime();
  const root = new Element("root");
  const options = ["Danmaku", "Subtitles", "Picture"].map((name) => ({ name, value: name, label: () => name }));
  const scope = new B.Scope();
  B.within(scope, () => B.Picker(root, { comp: "demo" }, { id: "tab", label: () => "Tab", field: B.cell("Danmaku"), options, style: "segmented", labelHidden: true }));
  const segmented = root.childNodes[0];
  assert.ok(segmented.classes.has("bern-segmented"));
  // Each segment holds its label alone, the middle track of its grid.
  for (const segment of segmented.childNodes) {
    assert.ok(segment.classes.has("bern-segment"));
    assert.deepEqual(segment.childNodes.map((child) => child.tagName), ["span"]);
  }
  scope.dispose();
  const features = readFileSync(new URL("../templates/features.css", import.meta.url), "utf8");
  const css = readFileSync(new URL("../templates/bern.css", import.meta.url), "utf8") + features;
  assert.match(css, /\.bern-scroll-vertical\s*\{\s*overflow-x:\s*hidden;/);
  assert.match(css, /\.bern-scroll-horizontal\s*\{\s*overflow-y:\s*hidden;/);
  assert.match(features.match(/\n\.bern-inspector \{[^}]*\}/)[0], /overflow:\s*hidden auto;/);
  assert.match(css.match(/\n\.bern-segmented \{[^}]*\}/)[0], /max-width:\s*100%;/);
  const segment = css.match(/\n\.bern-segment \{[^}]*\}/)[0];
  // 12 beside the label at full width: 4 of padding and an outer track of
  // 8 that gives way first; the label's track does not.
  assert.match(segment, /grid-template-columns:\s*minmax\(0, 8px\) minmax\(min-content, max-content\) minmax\(0, 8px\);/);
  assert.match(segment, /padding:\s*0 4px;/);
  assert.doesNotMatch(segment, /min-width/, "a fixed least width would keep the control from narrowing");
  // At least 56 wide with its 1 of border: 30 + 2 × 8 + 2 × 4 + 2.
  assert.match(css, /\.bern-segment > span\s*\{\s*grid-column:\s*2;\s*min-width:\s*30px;/);
});

test("a canvas with nothing to draw keeps no bitmap however it resizes, and draws once it has something", () => {
  const { B, F, Element, resizeObservers } = runtime();
  // The fixture's window, reached through a function made in it.
  F.Canvas.constructor("return globalThis")().devicePixelRatio = 2;
  const root = new Element("root");
  const ops = B.cell([]);
  const scope = new B.Scope();
  B.within(scope, () => F.Canvas(root, {}, {}, () => ops.get()));
  const node = root.childNodes[0];
  const canvas = node.childNodes[0];
  const cleared = [];
  canvas.getContext = () => ({ setTransform() {}, clearRect(...rect) { cleared.push(rect); } });
  const observer = resizeObservers.find((one) => one.targets.has(node));
  for (const width of [86, 120, 300]) {
    node.rect = { left: 0, top: 0, width, height: 15 };
    observer.callback([]);
  }
  assert.equal(canvas.width ?? 0, 0);
  assert.deepEqual(cleared, []);
  ops.set([{}]);
  assert.equal(canvas.width, 600);
  assert.equal(canvas.height, 30);
  assert.equal(cleared.length, 1);
  ops.set([]);
  assert.equal(canvas.width, 0);
  assert.equal(canvas.height, 0);
  scope.dispose();
});


test("continuous controlled slider commits during the drag without losing its preview", () => {
  const { B, Element } = runtime();
  const host = new Element("div");
  const current = B.cell(0.8);
  const commits = [];
  const scope = new B.Scope();
  const field = { get: () => current.get(), peek: () => current.peek(), set: value => { commits.push(value); current.set(value); } };
  B.within(scope, () => B.Slider(host, {}, { id: "opacity", label: () => "Opacity", field, min: 0, max: 1, controlled: true, continuous: true, labelHidden: true }));
  const input = host.children[0].children[0];
  input.events.pointerdown[0]();
  for (const value of [0.3, 0.4, 0.5]) {
    input.value = value;
    input.events.input[0]();
  }
  assert.deepEqual(commits, [0.3, 0.4, 0.5]);
  current.set(0.4); // An older engine response must not move the held thumb.
  assert.equal(Number(input.value), 0.5);
  input.value = 0.6;
  input.events.change[0]();
  assert.equal(commits.at(-1), 0.6);
  scope.dispose();
});

test("menus can hide disabled items and restore them without replacing the menu", () => {
  for (const hideDisabled of [false, true]) {
    const { B, Element, document } = runtime();
    const scope = new B.Scope();
    const root = new Element("root");
    const available = B.cell(false);
    B.within(scope, () => B.Menu(root, { comp: "player" }, { id: "more", label: () => "More", hideDisabled }, [
      { id: "chapters", label: () => "Chapters", enabled: () => available.get(), action() {} },
      { id: "info", label: () => "Information", action() {} },
    ]));
    const menu = root.children[0].children[1];
    const [chapters, info] = menu.children;
    assert.equal(chapters.disabled, true);
    assert.equal(chapters.style.display === "none", hideDisabled);
    menu.dispatch("keydown", { key: "Home" });
    assert.equal(info.focused, true, "keyboard navigation skips the unavailable entry");
    available.set(true);
    assert.equal(chapters.disabled, false);
    assert.notEqual(chapters.style.display, "none");
    document.activeElement = info;
    menu.dispatch("keydown", { key: "Home" });
    assert.equal(chapters.focused, true, "restored entries rejoin keyboard navigation");
    available.set(false);
    assert.equal(chapters.style.display === "none", hideDisabled);
    assert.equal(root.children[0].children[1], menu);
    scope.dispose();
    assert.equal(B.controlEntries().length, 0);
  }
});
