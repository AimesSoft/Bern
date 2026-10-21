// Bern Web runtime: drawing, motion, styles and the controls media
// applications need. The page imports it next to `bern.js`, whose cells,
// effects and DOM helpers it builds on.

import * as B from "./bern.js?v=__BERN_VERSION__";

const { cell, effect, onCleanup, el, untrack, eq } = B;

// MARK: - Motion

/// Whether the system asks for less motion: springs then settle at once
/// and transitions become a short fade.
const reduceQuery = matchMedia("(prefers-reduced-motion: reduce)");
const forceReduced = new URLSearchParams(location.search).get("motion") === "reduce";
const motionPreferences = new Map();
let reduced = reduceQuery.matches || forceReduced;
function updateReducedMotion() {
  reduced = reduceQuery.matches || forceReduced || [...motionPreferences.values()].some(Boolean);
}
reduceQuery.addEventListener("change", (event) => {
  updateReducedMotion();
});

export function reduceMotion() {
  return reduced;
}

/// Springs being driven, stepped together once per frame.
const running = new Set();
let lastFrame = 0;

function frame(now) {
  const elapsed = lastFrame ? Math.min(0.064, (now - lastFrame) / 1000) : 1 / 60;
  lastFrame = now;
  for (const spring of [...running]) spring.step(elapsed);
  if (running.size) requestAnimationFrame(frame);
  else lastFrame = 0;
}

/// A value that moves to its target as a spring of period `T` and
/// damping ratio `ζ` (mass 1): stiffness (2π / T)², damping 4πζ / T. A new
/// target starts from the current value and velocity, so motion can be
/// interrupted without a jump.
export class Spring {
  constructor(value, apply) {
    this.value = value;
    this.target = value;
    this.velocity = 0;
    this.apply = apply;
    this.done = null;
  }

  /// Jump to `value`, ending any motion.
  set(value) {
    running.delete(this);
    this.value = this.target = value;
    this.velocity = 0;
    this.apply(value);
    const done = this.done;
    this.done = null;
    done?.();
  }

  to(target, motion, done) {
    const previous = this.done;
    this.done = null;
    previous?.();
    if (!motion || reduced) {
      this.target = target;
      this.set(target);
      done?.();
      return;
    }
    const [period, damping] = motion;
    this.stiffness = (2 * Math.PI / period) ** 2;
    this.damping = (4 * Math.PI * damping) / period;
    this.target = target;
    this.done = done ?? null;
    this.epsilon = Math.max(1e-4, Math.abs(target - this.value) * 1e-4);
    if (Math.abs(target - this.value) < this.epsilon && Math.abs(this.velocity) < this.epsilon) {
      this.set(target);
      return;
    }
    running.add(this);
    if (running.size === 1) requestAnimationFrame(frame);
  }

  /// Give the spring a push, as a nudge does.
  kick(velocity, motion) {
    this.velocity += velocity;
    this.to(this.target, motion, this.done);
  }

  step(elapsed) {
    let left = elapsed;
    while (left > 0) {
      const h = Math.min(left, 1 / 240);
      const force = -this.stiffness * (this.value - this.target) - this.damping * this.velocity;
      this.velocity += force * h;
      this.value += this.velocity * h;
      left -= h;
    }
    if (Math.abs(this.value - this.target) < this.epsilon && Math.abs(this.velocity) < this.epsilon * 10) {
      this.set(this.target);
      return;
    }
    this.apply(this.value);
  }
}

/// The transform of an element is the sum of the moves and the product of
/// the scales and opacities of independent layers — the style's state,
/// an appearing transition, a shared element, reflow, a nudge, scroll
/// effects — so each can spring on its own. Scales are about the center.
class Layers {
  constructor(node) {
    this.node = node;
    this.layers = new Map();
    this.scheduled = false;
  }

  layer(name) {
    let layer = this.layers.get(name);
    if (!layer) {
      const write = () => this.write();
      layer = {
        x: new Spring(0, write), y: new Spring(0, write),
        sx: new Spring(1, write), sy: new Spring(1, write), opacity: new Spring(1, write),
      };
      this.layers.set(name, layer);
    }
    return layer;
  }

  write() {
    let x = 0;
    let y = 0;
    let sx = 1;
    let sy = 1;
    let opacity = 1;
    for (const layer of this.layers.values()) {
      x += layer.x.value;
      y += layer.y.value;
      sx *= layer.sx.value;
      sy *= layer.sy.value;
      opacity *= layer.opacity.value;
    }
    const style = this.node.style;
    const still = x === 0 && y === 0 && sx === 1 && sy === 1;
    style.transform = still ? "" : `translate(${x}px, ${y}px) scale(${sx}, ${sy})`;
    style.opacity = opacity === 1 ? "" : String(Math.max(0, Math.min(1, opacity)));
  }

  /// The scale and move all layers but `except` apply now.
  current(except) {
    let x = 0;
    let y = 0;
    let sx = 1;
    let sy = 1;
    for (const [name, layer] of this.layers) {
      if (name === except) continue;
      x += layer.x.value;
      y += layer.y.value;
      sx *= layer.sx.value;
      sy *= layer.sy.value;
    }
    return { x, y, sx, sy };
  }
}

export function layers(node) {
  if (!node.bernLayers) node.bernLayers = new Layers(node);
  return node.bernLayers;
}

/// Where `node` lies without the transforms Bern put on it, in the
/// viewport.
export function layoutRect(node) {
  const rect = node.getBoundingClientRect();
  const { x, y, sx, sy } = node.bernLayers ? node.bernLayers.current(null) : { x: 0, y: 0, sx: 1, sy: 1 };
  const width = rect.width / (sx || 1);
  const height = rect.height / (sy || 1);
  const cx = rect.left + rect.width / 2 - x;
  const cy = rect.top + rect.height / 2 - y;
  return { left: cx - width / 2, top: cy - height / 2, width, height };
}

/// A number that follows a getter, springing to each new value with
/// `motion` (or at once without one), written by `write`.
function bindNumber(value, motion, write) {
  if (value === undefined || value === null) return;
  if (typeof value !== "function") {
    write(value);
    return;
  }
  const spring = new Spring(Number(value()) || 0, write);
  write(spring.value);
  let first = true;
  effect(() => {
    const next = Number(value()) || 0;
    if (first) {
      first = false;
      return;
    }
    untrack(() => spring.to(next, motion));
  });
}

/// A length of `node`, such as its padding, bound to a getter. The
/// browser drops a negative padding or size, so a spring overshooting zero
/// stops there.
export function bindLength(node, property, value, motion) {
  if (value === undefined || value === null) return;
  // A side with a size of its own does not stretch with flexible content.
  if (property === "width") node.classList.add("bern-sized-width");
  if (property === "height") node.classList.add("bern-sized-height");
  bindNumber(value, motion, (v) => {
    node.style[property] = `${Math.max(0, v)}px`;
    // Children read their container's gap, as a toolbar group does.
    if (property === "gap") node.style.setProperty("--bern-gap", `${Math.max(0, v)}px`);
  });
}

/// The narrowest column of a `Grid`, bound to a getter: as many columns as
/// fit at that width.
export function bindGridMinWidth(node, value, motion) {
  bindNumber(value, motion, (v) => {
    node.style.gridTemplateColumns = `repeat(auto-fill, minmax(min(100%, ${Math.max(0, v)}px), 1fr))`;
    node.bernFitCells?.();
  });
}

/// Rows of a `Grid` as tall as the column width ÷ `ratio` + `extra`, its
/// items at their tops; content taller than that runs into the gap below.
export function bindGridCells(node, ratio, extra, motion) {
  node.classList.add("bern-grid-cells");
  let add = 0;
  const fit = () => {
    const column = parseFloat(getComputedStyle(node).gridTemplateColumns.split(" ")[0]) || 0;
    node.style.gridAutoRows = `${Math.max(0, column / ratio + add)}px`;
  };
  node.bernFitCells = fit;
  bindNumber(extra, motion, (v) => {
    add = v;
    fit();
  });
  new ResizeObserver(fit).observe(node);
}

// MARK: - Theme

/// Bumped when the colors behind role variables change: the system's
/// appearance or contrast, or the application's accent and appearance.
export const themeTick = cell(0);
for (const query of ["(prefers-color-scheme: dark)", "(prefers-contrast: more)"]) {
  matchMedia(query).addEventListener("change", () => themeTick.set(themeTick.peek() + 1));
}

/// The CSS color of a role (`accent`, `fill.subtle`) or a fixed pair, as
/// seen from `node`.
export function color(node, value) {
  if (!value) return null;
  if (value.role) {
    const name = `--bern-${value.role.replace(/[._]/g, "-")}`;
    return getComputedStyle(node).getPropertyValue(name).trim() || null;
  }
  return dark(node) ? value.dark : value.light;
}

/// Whether `node` shows the dark appearance.
export function dark(node) {
  return getComputedStyle(node).colorScheme.includes("dark");
}

/// A CSS `var(…)` for a role, for styles the browser keeps current.
export function roleVar(role) {
  return `var(--bern-${role.replace(/[._]/g, "-")})`;
}

// MARK: - Pictures

/// Pictures read at run time, and the average color of each, for shapes
/// filled from a picture. Each URL is read once.
const pictures = new Map();

function picture(url) {
  let entry = pictures.get(url);
  if (entry) return entry;
  entry = { average: cell(null) };
  pictures.set(url, entry);
  const image = new window.Image();
  if (/^https?:/.test(url) && !url.startsWith(location.origin)) image.crossOrigin = "anonymous";
  image.decoding = "async";
  image.onload = () => {
    try {
      // The mean over the whole picture: a single pixel samples its middle.
      const canvas = document.createElement("canvas");
      canvas.width = 32;
      canvas.height = 32;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      context.drawImage(image, 0, 0, 32, 32);
      const data = context.getImageData(0, 0, 32, 32).data;
      let red = 0;
      let green = 0;
      let blue = 0;
      for (let at = 0; at < data.length; at += 4) {
        red += data[at];
        green += data[at + 1];
        blue += data[at + 2];
      }
      const count = data.length / 4;
      entry.average.set(`rgb(${Math.round(red / count)} ${Math.round(green / count)} ${Math.round(blue / count)})`);
    } catch {
      // A picture from another origin without CORS cannot be read; the
      // shape keeps its `fill`.
    }
  };
  image.src = url;
  return entry;
}

/// The average color of the picture at `url`, once read; `null` before.
export function averageColor(url) {
  if (!url) return null;
  return picture(url).average.get();
}

/// A page-relative URL for a declared resource.
export function resource(name) {
  return `resources/${name.split("/").map(encodeURIComponent).join("/")}`;
}

// MARK: - Drawing

/// A canvas's frame: its declared size, filling the space otherwise.
function frameNode(width, height, animate) {
  const node = el("div", { class: "bern-canvas" });
  if (width === undefined) node.classList.add("bern-fill-width");
  if (height === undefined) node.classList.add("bern-fill-height");
  bindLength(node, "width", width, animate);
  bindLength(node, "height", height, animate);
  return node;
}

/// `Canvas`: shapes, series and patterns drawn in order, in CSS pixels.
/// `ops()` reads the state the drawing shows, so it redraws when that
/// changes, as it does when the canvas resizes or the colors change.
export function Canvas(p, ctx, { width, height, label, animate }, ops) {
  const node = frameNode(width, height, animate);
  const canvas = el("canvas", { class: "bern-canvas-surface" });
  node.append(canvas);
  if (label) {
    node.setAttribute("role", "img");
    effect(() => node.setAttribute("aria-label", label()));
  } else {
    node.setAttribute("aria-hidden", "true");
  }
  const size = cell({ width: 0, height: 0, ratio: devicePixelRatio });
  const observer = new ResizeObserver(() => {
    const rect = node.getBoundingClientRect();
    size.set({ width: node.clientWidth || rect.width, height: node.clientHeight || rect.height, ratio: devicePixelRatio });
  });
  observer.observe(node);
  onCleanup(() => observer.disconnect());
  effect(() => {
    const list = ops();
    // Nothing to draw, nothing drawn: a canvas that only holds a place
    // keeps no bitmap, however often it resizes.
    if (!list.length) {
      if (canvas.width || canvas.height) canvas.width = canvas.height = 0;
      return;
    }
    const { width: w, height: h, ratio } = size.get();
    themeTick.get();
    if (!w || !h) return;
    canvas.width = Math.round(w * ratio);
    canvas.height = Math.round(h * ratio);
    const context = canvas.getContext("2d");
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, w, h);
    for (const op of list) {
      if (op.shape) drawShape(context, node, op, w, h);
      else if (op.series) drawSeries(context, node, op, w, h);
      else if (op.pattern) drawPattern(context, node, op, w, h);
    }
  });
  p.append(node);
}

function roundedPath(context, x, y, width, height, radius) {
  const r = Math.max(0, Math.min(radius, width / 2, height / 2));
  context.beginPath();
  context.roundRect(x, y, width, height, r);
}

function drawShape(context, node, op, w, h) {
  const x = op.x ?? 0;
  const y = op.y ?? 0;
  const width = op.width ?? w - x;
  const height = op.height ?? h - y;
  const radius = op.shape === "rounded" ? op.corner ?? 8 : op.shape === "capsule" ? Math.min(width, height) / 2 : 0;
  const path = () => {
    if (op.shape === "ellipse") {
      context.beginPath();
      context.ellipse(x + width / 2, y + height / 2, Math.max(0, width / 2), Math.max(0, height / 2), 0, 0, Math.PI * 2);
    } else {
      roundedPath(context, x, y, width, height, radius);
    }
  };
  context.globalAlpha = op.opacity ?? 1;
  const filled = op.fillFrom ?? (op.fill || !op.stroke ? color(node, op.fill ?? { role: "fill.strong" }) : null);
  if (filled) {
    if (op.fade) {
      const [x0, y0, x1, y1] = {
        up: [x, y + height, x, y], down: [x, y, x, y + height],
        left: [x + width, y, x, y], right: [x, y, x + width, y],
      }[op.fade];
      const gradient = context.createLinearGradient(x0, y0, x1, y1);
      gradient.addColorStop(0, filled);
      gradient.addColorStop(1, transparent(filled));
      context.fillStyle = gradient;
    } else {
      context.fillStyle = filled;
    }
    path();
    context.fill();
  }
  if (op.stroke) {
    context.strokeStyle = color(node, op.stroke);
    context.lineWidth = op.strokeWidth ?? 1;
    // Dashes `dash` long with gaps of `dashGap`, by default as long.
    context.setLineDash(op.dash ? [op.dash, op.dashGap ?? op.dash] : []);
    path();
    context.stroke();
    context.setLineDash([]);
  }
  context.globalAlpha = 1;
}

/// The same color with no opacity, for a fill that fades out.
function transparent(css) {
  const probe = document.createElement("canvas").getContext("2d");
  probe.fillStyle = css;
  const value = probe.fillStyle;
  if (value.startsWith("#")) {
    const n = parseInt(value.slice(1), 16);
    return `rgb(${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255} / 0)`;
  }
  return value.replace(/rgba?\(([^)]*)\)/, (_, inner) => {
    const parts = inner.split(/[\s,/]+/).filter(Boolean).slice(0, 3);
    return `rgb(${parts.join(" ")} / 0)`;
  });
}

/// Values across the full width, scaled so `max` (or the largest value)
/// reaches the top.
function drawSeries(context, node, op, w, h) {
  const values = (op.series ?? []).map(Number);
  if (!values.length) return;
  const peak = op.max ?? Math.max(...values);
  const scale = peak > 0 ? peak : 1;
  const step = values.length > 1 ? w / (values.length - 1) : 0;
  const points = values.map((value, index) => [
    values.length > 1 ? index * step : w / 2,
    h - h * Math.min(1, Math.max(0, value / scale)),
  ]);
  const line = () => {
    context.beginPath();
    context.moveTo(...points[0]);
    for (const point of points.slice(1)) context.lineTo(...point);
  };
  context.globalAlpha = op.opacity ?? 1;
  context.lineJoin = "round";
  context.lineCap = "round";
  context.lineWidth = op.strokeWidth ?? 2;
  if (op.area) {
    line();
    context.lineTo(points[points.length - 1][0], h);
    context.lineTo(points[0][0], h);
    context.closePath();
    context.fillStyle = color(node, op.fill ?? { role: "accent" });
    context.fill();
    if (op.stroke) {
      line();
      context.strokeStyle = color(node, op.stroke);
      context.stroke();
    }
  } else {
    line();
    context.strokeStyle = color(node, op.stroke ?? { role: "accent" });
    context.stroke();
  }
  context.globalAlpha = 1;
}

/// A woven check: the bands repeat across and down, and where colored
/// bands cross both show; with `twill`, fine diagonal lines over it.
function drawPattern(context, node, op, w, h) {
  const pattern = op.pattern;
  const sett = pattern.bands.reduce((sum, band) => sum + Math.max(band.width, 0), 0);
  if (sett <= 0) return;
  const alpha = op.opacity ?? 1;
  for (const across of [true, false]) {
    const extent = across ? w : h;
    let offset = 0;
    while (offset < extent) {
      for (const band of pattern.bands) {
        if (band.width <= 0) continue;
        if (band.color) {
          context.globalAlpha = alpha * band.opacity;
          context.fillStyle = color(node, band.color);
          if (across) context.fillRect(offset, 0, band.width, h);
          else context.fillRect(0, offset, w, band.width);
        }
        offset += band.width;
        if (offset >= extent) break;
      }
    }
  }
  if (pattern.twill) {
    context.globalAlpha = alpha * 0.05;
    context.strokeStyle = color(node, { role: "text.primary" });
    context.lineWidth = 0.5;
    context.beginPath();
    for (let start = -h; start < w; start += 3) {
      context.moveTo(start, h);
      context.lineTo(start + h, 0);
    }
    context.stroke();
  }
  context.globalAlpha = 1;
}

// MARK: - Text marks

/// `Text(mark:)`: a capsule of the mark color under each line of the text,
/// from 74 % to 98 % of the line's height and 0.4 em longer than the line,
/// drawn behind the text without taking space.
export function markText(node, text, role) {
  node.classList.add("bern-marked");
  const layer = el("span", { class: "bern-marks", "aria-hidden": "true" });
  node.prepend(layer);
  const draw = () => {
    layer.replaceChildren();
    if (!text.firstChild) return;
    const range = document.createRange();
    range.selectNodeContents(text);
    const origin = node.getBoundingClientRect();
    const em = parseFloat(getComputedStyle(node).fontSize) || 16;
    for (const rect of range.getClientRects()) {
      if (!rect.width) continue;
      const band = rect.height * 0.24;
      const mark = el("span", { class: "bern-mark" });
      mark.style.left = `${rect.left - origin.left}px`;
      mark.style.top = `${rect.top - origin.top + rect.height * 0.74}px`;
      mark.style.width = `${rect.width + em * 0.4}px`;
      mark.style.height = `${band}px`;
      mark.style.borderRadius = `${band / 2}px`;
      mark.style.background = roleVar(role);
      layer.append(mark);
    }
  };
  const observer = new ResizeObserver(draw);
  observer.observe(node);
  onCleanup(() => observer.disconnect());
  document.fonts?.ready.then(draw);
  return draw;
}

export function MarkedText(p, ctx, { content, style, emphasis, mark }) {
  const node = el("p", { class: `bern-text bern-${B.STYLE_CLASS[style] ?? "body"} bern-emphasis-${emphasis ?? "primary"}` });
  const text = el("span", { class: "bern-marked-text" });
  node.append(text);
  const draw = markText(node, text, mark);
  effect(() => {
    text.textContent = content();
    requestAnimationFrame(draw);
  });
  p.append(node);
}

// MARK: - Styles

/// The text color a fill pairs with.
function pairOf(fill) {
  if (!fill?.role) return null;
  if (fill.role === "accent") return { role: "on_accent" };
  if (fill.role === "status.error") return { role: "on_error" };
  if (fill.role === "selection.background") return { role: "selection.foreground" };
  // Neutral surfaces and state fills use the theme's primary text. In
  // particular, a styled sidebar may replace its accent background with
  // fill.strong; retaining the default on_accent would make dark text sit
  // on a dark neutral fill. These pairings are checked by the theme kernel.
  if (["fill.subtle", "fill.strong", "surface.canvas", "surface.panel", "surface.raised"].includes(fill.role)) {
    return { role: "text.primary" };
  }
  return null;
}

function cssColor(value) {
  if (!value) return "";
  if (value.role) return roleVar(value.role);
  return `light-dark(${value.light}, ${value.dark})`;
}

const INDICATOR = {
  leading: (w) => `inset ${w}px 0 0 0`, trailing: (w) => `inset -${w}px 0 0 0`,
  top: (w) => `inset 0 ${w}px 0 0`, bottom: (w) => `inset 0 -${w}px 0 0`,
};

/// A form style's background is its rows': the form itself shows the
/// page around it.
export function formRows(node, style) {
  const row = style?.base?.background;
  if (row) node.style.setProperty("--bern-form-row", cssColor(row));
}

/// A `style!` override on a control: its base look, and the looks of its
/// states — hover, pressed, focus, disabled, selected — merged in that
/// order, later ones winning. Colors change with a short CSS transition;
/// scale, offset and opacity spring with the state's motion and settle
/// back with `settle`.
export function applyStyle(node, style, { selected } = {}) {
  node.classList.add("bern-styled");
  if (style.draws) node.classList.add("bern-styled-look");
  if (style.size) node.classList.add(`bern-size-${style.size}`);
  if (style.corner !== undefined) {
    node.style.borderRadius = `${style.corner}px`;
    // A stack's or a content button's corner clips what it holds, as on the
    // other targets: a picture, a progress bar, a person's shoulders.
    if (node.classList.contains("bern-layers") || node.classList.contains("bern-content-button")) node.style.overflow = "clip";
  }
  if (style.padding !== undefined) node.style.padding = `${style.padding}px`;
  const state = { hover: false, pressed: false, focus: false, disabled: false, selected: false };
  const layer = layers(node).layer("state");
  let lastMotion = null;
  const update = () => {
    const looks = [style.base];
    for (const name of ["selected", "hover", "pressed", "focus", "disabled"]) {
      if (state[name] && style[name]) looks.push(style[name]);
    }
    const look = Object.assign({}, ...looks);
    // The browser's own hover and pressed looks stay unless a state says
    // `none`, or the style draws the control.
    node.classList.toggle("bern-styled-quiet", looks.some((one) => one.quiet));
    const fill = look.fill;
    node.style.background = cssColor(fill ?? look.background);
    // Text on a fill or background pairs with it unless the style says.
    const foreground = cssColor(look.foreground ?? pairOf(fill ?? look.background));
    node.style.color = foreground;
    // Icons inside follow the text color the style sets.
    node.classList.toggle("bern-styled-foreground", foreground !== "");
    node.style.border = look.border ? `${look.borderWidth ?? 1}px solid ${cssColor(look.border)}` : "";
    const shadows = [];
    if (look.shadow) {
      const [radius, y, opacity] = look.shadow;
      shadows.push(`0 ${y}px ${radius}px rgb(0 0 0 / ${opacity})`);
    }
    if (look.indicator) shadows.push(`${INDICATOR[look.indicatorEdge ?? "leading"](look.indicatorWidth ?? 3)} ${cssColor(look.indicator)}`);
    node.style.boxShadow = shadows.join(", ");
    // The motion of the topmost state that moves; back at rest, `settle`.
    const moving = [...looks].reverse().find((one) => one.motion);
    const motion = moving ? moving.motion : style.settle ?? lastMotion;
    if (moving) lastMotion = moving.motion;
    const to = (spring, value) => {
      if (spring.target !== value) spring.to(value, motion);
    };
    to(layer.sx, look.scaleX ?? 1);
    to(layer.sy, look.scaleY ?? 1);
    to(layer.x, look.offsetX ?? 0);
    to(layer.y, look.offsetY ?? 0);
    to(layer.opacity, look.opacity ?? 1);
  };
  const set = (name, value) => {
    if (state[name] === value) return;
    state[name] = value;
    update();
  };
  node.addEventListener("pointerenter", (event) => {
    if (event.pointerType === "mouse") set("hover", true);
  });
  node.addEventListener("pointerleave", () => {
    set("hover", false);
    set("pressed", false);
  });
  node.addEventListener("pointerdown", (event) => {
    if (event.button === 0) set("pressed", true);
  });
  for (const name of ["pointerup", "pointercancel"]) node.addEventListener(name, () => set("pressed", false));
  node.addEventListener("keydown", (event) => {
    if (event.key === " " || event.key === "Enter") set("pressed", true);
  });
  node.addEventListener("keyup", () => set("pressed", false));
  node.addEventListener("focusin", () => set("focus", node.matches(":focus-visible")));
  node.addEventListener("focusout", () => set("focus", false));
  const disabled = new MutationObserver(() => set("disabled", node.disabled === true));
  disabled.observe(node, { attributes: true, attributeFilter: ["disabled"] });
  onCleanup(() => disabled.disconnect());
  if (selected) effect(() => set("selected", selected()));
  update();
}

B.hooks.style = applyStyle;
B.hooks.length = (node, property, value, motion) => bindLength(node, property, value, motion);
B.hooks.accepts = (url, kinds) => accepts(url, "", kinds);
B.hooks.spring = (value, apply) => new Spring(value, apply);
B.hooks.reduced = reduceMotion;

// MARK: - Split view and sidebar

/// The widths a person drags a sidebar of no declared width between, as
/// the Web design gives them: 232 to start, 200 to 320, in steps of 10
/// from the keyboard.
export const SIDEBAR_WIDTHS = { initial: 232, min: 200, max: 320, step: 10 };
/// How long the sidebar takes to slide away or back.
const SIDEBAR_FOLD = 0.25;
let sidebarSequence = 0;

/// A sidebar and detail split whose items may come and go with `if`,
/// `match` and `ForEach`: each item registers while it is mounted. When
/// the selected item disappears, the selection moves to `fallback` or the
/// first item left. On narrow windows the sidebar is the first page.
///
/// On wide windows a button leading the detail's page header hides the
/// whole sidebar and shows it again, as the toolbar's button does on
/// macOS; the button stays where it is, so focus stays on it. A sidebar
/// of no declared width has a Window Splitter over its edge that drags or
/// steps it between `SIDEBAR_WIDTHS`; a declared width stays fixed.
export function SplitView(p, ctx, spec, sidebarContent, detail) {
  const { selection, fallback, search, footer, header: sidebarHeader, width, titled, id } = spec;
  const showingDetail = cell(false);
  const collapsed = cell(false);
  const list = el("div", { class: "bern-sidebar-list", role: "listbox", tabindex: "0" });
  const sidebarId = `bern-sidebar-${++sidebarSequence}`;
  const sidebar = el("nav", { class: "bern-sidebar", id: sidebarId });
  const dragged = cell(SIDEBAR_WIDTHS.initial);
  effect(() => sidebar.style.setProperty("--bern-sidebar-width", `${width === undefined ? dragged.get() : Number(typeof width === "function" ? width() : width)}px`));
  if (sidebarHeader) {
    const holder = el("div", { class: "bern-sidebar-header" });
    sidebarHeader(holder, ctx);
    sidebar.append(holder);
  }
  if (search) {
    const holder = el("div", { class: "bern-sidebar-search" });
    search(holder, ctx);
    sidebar.append(holder);
  }
  sidebar.append(list);
  const registry = {
    items: cell([]),
    selection,
    choose(value) {
      selection.set(value);
      showingDetail.set(true);
    },
  };
  sidebarContent(list, { ...ctx, sidebar: registry });
  if (footer) {
    const holder = el("div", { class: "bern-sidebar-footer" });
    footer(holder, ctx);
    sidebar.append(holder);
  }
  /// Mounted items in the order they show.
  const ordered = () => [...registry.items.get()].sort((a, b) =>
    a.option.compareDocumentPosition(b.option) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1);
  // A selection without a mounted item moves on, once the update that
  // removed it has mounted whatever replaces it.
  const settle = () => {
    const shown = registry.items.peek();
    if (!shown.length || shown.some((item) => eq(item.value, selection.peek()))) return;
    const back = fallback !== undefined && shown.find((item) => eq(item.value, fallback));
    selection.set((back || ordered()[0]).value);
  };
  effect(() => {
    registry.items.get();
    selection.get();
    queueMicrotask(settle);
  });
  list.addEventListener("keydown", (event) => {
    const items = ordered();
    const index = items.findIndex((item) => eq(item.value, selection.peek()));
    const move = { ArrowDown: 1, ArrowUp: -1, Home: -Infinity, End: Infinity }[event.key];
    if (move === undefined || !items.length) return;
    event.preventDefault();
    const next = Math.min(items.length - 1, Math.max(0, move === -Infinity ? 0 : move === Infinity ? items.length - 1 : index + move));
    selection.set(items[next].value);
  });
  const selectedLabel = () => {
    const value = selection.get();
    return registry.items.get().find((item) => eq(item.value, value))?.label?.() ?? "";
  };
  const select = (name) => {
    const found = registry.items.peek().find((item) => item.name === name);
    if (!found) return false;
    registry.choose(found.value);
    return true;
  };
  if (id) {
    B.mark(list, ctx, id);
    B.control(ctx, {
      target: () => B.target(ctx, id), role: "sidebar", label: selectedLabel,
      actions: { select, value: () => registry.items.peek().find((item) => eq(item.value, selection.peek()))?.name ?? null },
    });
  }
  const inner = { ...ctx, page: B.page() };
  const header = B.pageHeader(inner, () => titled && !titled() ? "" : selectedLabel(), true);
  const back = el("button", { class: "bern-icon-button bern-split-back", type: "button" }, B.icon("chevron.left", 18));
  B.bindAttr(back, "aria-label", () => B.tr("bern.navigation.back"));
  back.addEventListener("click", () => showingDetail.set(false));
  const toggle = el("button", { class: "bern-icon-button bern-split-toggle", type: "button", "aria-controls": sidebarId }, B.icon("sidebar.left", 18));
  const toggleLabel = () => B.tr(collapsed.get() ? "bern.navigation.show_sidebar" : "bern.navigation.hide_sidebar");
  B.bindAttr(toggle, "aria-label", toggleLabel);
  B.bindAttr(toggle, "title", toggleLabel);
  toggle.addEventListener("click", () => {
    node.style.setProperty("--bern-sidebar-fold", `${reduceMotion() ? 0 : SIDEBAR_FOLD}s`);
    collapsed.set(!collapsed.peek());
  });
  header.prepend(toggle);
  header.prepend(back);
  const body = el("div", { class: "bern-detail-body" });
  const column = el("div", { class: "bern-detail" }, header, body);
  detail(body, inner);
  const node = el("div", { class: "bern-split" }, sidebar);
  if (width === undefined) node.append(splitter(node, sidebarId, dragged));
  node.append(column);
  effect(() => node.classList.toggle("bern-split-detail", showingDetail.get()));
  effect(() => node.classList.toggle("bern-split-collapsed", collapsed.get()));
  p.append(node);
}

/// The Window Splitter over a sidebar's trailing edge: an invisible strip
/// with the resize cursor that a pointer drags and the arrow keys step,
/// Home and End taking it to the narrowest and widest. Its value is the
/// sidebar's width; right to left, the sidebar grows to the left.
function splitter(split, sidebarId, dragged) {
  const { min, max, step } = SIDEBAR_WIDTHS;
  const handle = el("div", {
    class: "bern-split-handle", role: "separator", tabindex: "0", "aria-orientation": "vertical",
    "aria-controls": sidebarId, "aria-valuemin": String(min), "aria-valuemax": String(max),
  });
  B.bindAttr(handle, "aria-label", () => B.tr("bern.navigation.sidebar"));
  effect(() => handle.setAttribute("aria-valuenow", String(dragged.get())));
  const resize = (to) => dragged.set(Math.round(Math.min(max, Math.max(min, to))));
  const outward = () => getComputedStyle(split).direction === "rtl" ? -1 : 1;
  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    handle.focus?.({ preventScroll: true });
    const from = event.clientX;
    const start = dragged.peek();
    const sign = outward();
    handle.setPointerCapture(event.pointerId);
    split.classList.add("bern-split-resizing");
    const move = (moved) => resize(start + sign * (moved.clientX - from));
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("lostpointercapture", end);
      split.classList.remove("bern-split-resizing");
    };
    handle.addEventListener("pointermove", move);
    // Capture ends with the button's release or a cancelled pointer.
    handle.addEventListener("lostpointercapture", end);
  });
  handle.addEventListener("keydown", (event) => {
    const width = dragged.peek();
    const to = {
      ArrowLeft: width - outward() * step, ArrowRight: width + outward() * step, Home: min, End: max,
    }[event.key];
    if (to === undefined) return;
    event.preventDefault();
    resize(to);
  });
  return handle;
}


/// A titled group of sidebar items.
export function SidebarSection(p, ctx, { title }, children) {
  const group = el("div", { class: "bern-sidebar-section", role: "group" });
  if (title) {
    const heading = el("div", { class: "bern-sidebar-heading" });
    B.bindText(heading, title);
    group.append(heading);
  }
  children(group, ctx);
  p.append(group);
}

/// One sidebar destination; it is part of the selection while mounted.
export function SidebarItem(p, ctx, { value, name, label, icon: glyph, style, badge }) {
  const registry = ctx.sidebar;
  const text = el("span");
  if (label) B.bindText(text, label);
  const option = el("div", { class: "bern-sidebar-item", role: "option" }, glyph ? B.icon(glyph, 16) : null, text);
  text.classList.add("bern-sidebar-label");
  if (badge !== undefined) {
    const counter = el("span", { class: "bern-sidebar-badge" });
    effect(() => {
      const value = typeof badge === "function" ? badge() : badge;
      counter.textContent = String(value);
      counter.hidden = Number(value) <= 0;
    });
    option.append(counter);
  }
  option.addEventListener("click", () => registry.choose(value));
  const entry = { value, name, label, option };
  registry.items.set([...registry.items.peek(), entry]);
  onCleanup(() => registry.items.set(registry.items.peek().filter((item) => item !== entry)));
  const selected = () => eq(registry.selection.get(), value);
  effect(() => option.setAttribute("aria-selected", selected() ? "true" : "false"));
  if (style) applyStyle(option, style, { selected });
  p.append(option);
}

// MARK: - Structural motion

const QUICK_FADE = [0.15, 1];

/// The start (or end) of a transition as layer values.
function transitionFrom(transition, node) {
  const from = { x: 0, y: 0, sx: 1, sy: 1, opacity: 0 };
  if (transition.kind === "pop") {
    from.sx = from.sy = transition.from;
  } else if (transition.kind === "grow") {
    from.opacity = 1;
    const edge = transition.edge;
    const rtl = getComputedStyle(node).direction === "rtl";
    const vertical = edge === "top" || edge === "bottom";
    node.style.transformOrigin = vertical ? `center ${edge}` : `${edge === "leading" ? (rtl ? "right" : "left") : (rtl ? "left" : "right")} center`;
    from[vertical ? "sy" : "sx"] = transition.from ?? 0;
  } else if (transition.kind === "slide") {
    const distance = transition.distance;
    const rtl = getComputedStyle(node).direction === "rtl";
    const edge = transition.edge;
    if (edge === "top") from.y = -distance;
    else if (edge === "bottom") from.y = distance;
    else if (edge === "leading") from.x = rtl ? distance : -distance;
    else if (edge === "trailing") from.x = rtl ? -distance : distance;
  }
  return from;
}

function playTransition(node, transition, entering, done, delay = 0) {
  const layer = layers(node).layer(entering ? "enter" : "exit");
  // With less motion, only a short fade.
  const motion = reduced ? QUICK_FADE : transition.motion;
  const from = reduced ? { x: 0, y: 0, sx: 1, sy: 1, opacity: 0 } : transitionFrom(transition, node);
  const start = entering ? from : { x: 0, y: 0, sx: 1, sy: 1, opacity: 1 };
  const end = entering ? { x: 0, y: 0, sx: 1, sy: 1, opacity: 1 } : from;
  for (const key of ["x", "y", "sx", "sy", "opacity"]) layer[key].set(start[key]);
  let left = 5;
  const finish = () => {
    if (--left === 0) done?.();
  };
  const play = () => {
    // A fade with less motion still moves; the spring settles quickly.
    for (const key of ["x", "y", "sx", "sy", "opacity"]) {
      if (reduced && key === "opacity") {
        fade(layer.opacity, start.opacity, end.opacity, 150, finish);
      } else {
        layer[key].to(end[key], motion, finish);
      }
    }
  };
  // A staggered element waits at its start, hidden, for its turn.
  if (delay > 0 && !reduced) setTimeout(play, delay * 1000);
  else play();
}

// MARK: - Stagger

/// The stagger clock of the container being built: its direct children
/// that appear take turns, `step` seconds apart. The count starts over
/// with each update. `play` says whether the container itself appeared in
/// an update, so its children play `appear` with it.
let clock = null;

function makeClock(step) {
  return { step, next: 0, resetting: false, play: false };
}

/// The delay of the next child to appear on `turns`.
function turn(turns) {
  const delay = turns.next * turns.step;
  turns.next += 1;
  if (!turns.resetting) {
    turns.resetting = true;
    queueMicrotask(() => {
      turns.next = 0;
      turns.resetting = false;
    });
  }
  return delay;
}

function withClock(next, body) {
  const saved = clock;
  clock = next;
  try {
    return body();
  } finally {
    clock = saved;
  }
}

B.hooks.clock = () => clock;
B.hooks.makeClock = makeClock;
B.hooks.withClock = withClock;

/// The children `body` builds are the staggering children of a container.
export function staggered(step, body) {
  const turns = makeClock(step);
  turns.play = B.isUpdating();
  return withClock(turns, body);
}

/// A plain timed fade, for reduced motion.
function fade(spring, from, to, duration, done) {
  const begin = performance.now();
  const step = (now) => {
    const t = Math.min(1, (now - begin) / duration);
    spring.set(from + (to - from) * t);
    if (t < 1) requestAnimationFrame(step);
    else done?.();
  };
  spring.set(from);
  requestAnimationFrame(step);
}

/// Waits for the disappearing transitions of removed nodes before they
/// leave the page.
B.hooks.exit = (node, done) => {
  if (!node.bernDisappear) return false;
  node.bernExiting = true;
  node.style.pointerEvents = "none";
  playTransition(node, node.bernDisappear, false, done);
  return true;
};

// MARK: - Shared elements

/// Elements that carry a `hero` key while mounted, and the last place of
/// those that just left, for the next one with the same key to grow from.
const heroes = new Map();
const departed = new Map();
const heroFlights = new Map();

function heroLeave(key, node) {
  const active = heroes.get(key) ?? [];
  const remaining = active.filter((item) => item !== node && item.isConnected);
  if (remaining.length) heroes.set(key, remaining); else heroes.delete(key);
  if (!node.isConnected) return;
  const rect = node.getBoundingClientRect();
  if (!rect.width || !rect.height) return;
  departed.set(key, { rect, node, ghost: ghostOf(node) });
  setTimeout(() => {
    if (departed.get(key)?.node === node) departed.delete(key);
    const flight = heroFlights.get(key);
    if (flight?.node === node && !node.isConnected) flight.cancel();
  }, 0);
}

/// A copy of `node` for the cross-fade, keeping the text look it inherits.
/// Media and drawings do not copy, so they get no ghost.
function ghostOf(node) {
  if (node.querySelector?.("video, canvas") || node.matches?.("video, canvas")) return null;
  const ghost = node.cloneNode(true);
  const style = getComputedStyle(node);
  ghost.style.font = style.font;
  ghost.style.color = style.color;
  ghost.style.letterSpacing = style.letterSpacing;
  ghost.style.borderRadius = style.borderRadius;
  ghost.style.margin = "0";
  ghost.removeAttribute("data-bern-target");
  ghost.setAttribute("aria-hidden", "true");
  for (const marked of ghost.querySelectorAll?.("[data-bern-target]") ?? []) marked.removeAttribute("data-bern-target");
  return ghost;
}

function heroEnter(key, node, motion) {
  const active = heroes.get(key) ?? [];
  const previous = active.at(-1);
  heroes.set(key, [...active.filter((item) => item !== node && item.isConnected), node]);
  queueMicrotask(() => {
    let from = departed.get(key) ?? (previous && previous !== node && previous.isConnected
      ? { rect: previous.getBoundingClientRect(), node: previous, ghost: ghostOf(previous) } : null);
    const flight = heroFlights.get(key);
    if (flight) { from = flight.snapshot(); flight.cancel(); }
    if (!from || !node.isConnected || reduced) return;
    departed.delete(key);
    // ScaleBox receives its first size from ResizeObserver, after the
    // mounting microtask. Keep the destination hidden until it has a
    // usable viewport rectangle; never discard a flight for a zero size.
    const visibility = node.style.visibility;
    node.style.visibility = "hidden";
    let attempts = 0;
    const ready = () => {
      if (!node.isConnected || reduced) { node.style.visibility = visibility; return; }
      const rect = node.getBoundingClientRect();
      if ((!rect.width || !rect.height) && ++attempts < 8) { requestAnimationFrame(ready); return; }
      node.style.visibility = visibility;
      if (!rect.width || !rect.height) return;
      if (node.matches("img") && from.ghost?.matches("img")) growPicture(key, node, from, motion);
      else grow(node, from, motion);
    };
    ready();
  });
}

/// Images fly entirely in viewport space. Transforming the real image
/// inside a clipped sidebar or a scaled scene clips/double-scales it.
function growPicture(key, node, from, motion) {
  const to = node.getBoundingClientRect();
  const ghost = ghostOf(node);
  const old = from.ghost;
  const a = from.rect;
  const radius = (element, rect) => (parseFloat(element.style.borderRadius) || 0)
    * rect.width / (parseFloat(element.style.width) || rect.width);
  const r0 = radius(old, a), r1 = radius(ghost, to);
  for (const picture of [old, ghost]) {
    Object.assign(picture.style, { position: "fixed", margin: "0", maxWidth: "none", maxHeight: "none",
      minWidth: "0", minHeight: "0", transform: "none", visibility: "visible", pointerEvents: "none",
      zIndex: "2147483000", boxSizing: "border-box" });
    document.body.append(picture);
  }
  const sourceVisibility = from.node?.style.visibility;
  const destinationVisibility = node.style.visibility;
  if (from.node) from.node.style.visibility = "hidden";
  node.style.visibility = "hidden";
  let stopped = false;
  const cleanup = () => {
    if (stopped) return;
    stopped = true;
    old.remove(); ghost.remove();
    if (from.node) from.node.style.visibility = sourceVisibility;
    node.style.visibility = destinationVisibility;
    if (heroFlights.get(key)?.node === node) heroFlights.delete(key);
  };
  const progress = new Spring(0, (t) => {
    if (!node.isConnected || reduced) { running.delete(progress); progress.done = null; cleanup(); return; }
    const mix = (start, end) => start + (end - start) * t;
    for (const picture of [old, ghost]) Object.assign(picture.style, {
      left: `${mix(a.left, to.left)}px`, top: `${mix(a.top, to.top)}px`,
      width: `${Math.max(1, mix(a.width, to.width))}px`, height: `${Math.max(1, mix(a.height, to.height))}px`,
      borderRadius: `${Math.max(0, mix(r0, r1))}px`,
    });
    old.style.opacity = "1";
    ghost.style.opacity = String(Math.max(0, Math.min(1, t)));
  });
  heroFlights.set(key, { node, snapshot: () => ({ rect: ghost.getBoundingClientRect(), ghost: ghostOf(ghost), node }),
    cancel: () => { running.delete(progress); progress.done = null; cleanup(); } });
  progress.apply(0);
  progress.to(1, motion, cleanup);
}

/// The new element grows from the old one's place and size while the old
/// one's copy moves along and fades out: one spring drives both.
function grow(node, from, motion) {
  const to = layoutRect(node);
  if (!to.width || !to.height || reduced) return;
  const a = from.rect;
  const dx = a.left + a.width / 2 - (to.left + to.width / 2);
  const dy = a.top + a.height / 2 - (to.top + to.height / 2);
  const sx = a.width / to.width;
  const sy = a.height / to.height;
  const layer = layers(node).layer("hero");
  let ghost = null;
  if (from.ghost) {
    ghost = from.ghost;
    Object.assign(ghost.style, {
      position: "fixed", left: `${a.left}px`, top: `${a.top}px`, width: `${a.width}px`, height: `${a.height}px`,
      transform: "", opacity: "1", pointerEvents: "none", zIndex: "2147483000", boxSizing: "border-box",
    });
    document.body.append(ghost);
  }
  const progress = new Spring(0, (t) => {
    const u = 1 - t;
    layer.x.set(dx * u);
    layer.y.set(dy * u);
    layer.sx.set(1 + (sx - 1) * u);
    layer.sy.set(1 + (sy - 1) * u);
    layer.opacity.set(Math.max(0, Math.min(1, t)));
    if (ghost) {
      ghost.style.transform = `translate(${-dx * t}px, ${-dy * t}px) scale(${1 + (1 / sx - 1) * t}, ${1 + (1 / sy - 1) * t})`;
      ghost.style.opacity = String(Math.max(0, Math.min(1, 1 - t)));
    }
  });
  progress.apply(0);
  progress.to(1, motion, () => ghost?.remove());
}

// MARK: - Element decoration

/// The motion and appearance props any element takes, applied to the
/// element `build` makes: `appear` and `disappear` transitions, a `hero`
/// key, `reveal: hover`, `appearance`, `nudge`, and following scroll with
/// `scrollScale` and `scrollOpacity`.
export function Decorate(p, ctx, spec, build) {
  if (spec.reduceMotion) {
    const key = {};
    effect(() => {
      motionPreferences.set(key, Boolean(spec.reduceMotion()));
      updateReducedMotion();
    });
    onCleanup(() => {
      motionPreferences.delete(key);
      updateReducedMotion();
    });
  }
  const fragment = document.createDocumentFragment();
  // Elements inside this one enter with it, not on their own; a staggering
  // container's clock is only for its direct children.
  const animated = B.isEntering();
  const turns = clock;
  B.structural(false, () => withClock(null, () => build(fragment, ctx)));
  const nodes = [...fragment.childNodes].filter((node) => node.nodeType === Node.ELEMENT_NODE);
  for (const node of nodes) {
    if (spec.appearance) node.classList.add(`bern-appearance-${spec.appearance}`);
    if (spec.idleArea !== undefined) {
      node.classList.add("bern-idle-area");
      node.dataset.idleAfter = spec.idleArea;
    }
    if (spec.reveal) {
      node.classList.add("bern-reveal");
      queueMicrotask(() => node.parentElement?.classList.add("bern-reveal-host"));
    }
    if (spec.disappear) node.bernDisappear = spec.disappear;
    if (spec.appear && (animated || turns?.play || spec.appearDelay !== undefined)) {
      playTransition(node, spec.appear, true, null, (turns ? turn(turns) : 0) + (spec.appearDelay ?? 0));
    }
    if (spec.hero) {
      const key = spec.hero;
      let current = null;
      effect(() => {
        const next = `${ctx?.win?.id ?? "root"}:${key()}`;
        untrack(() => {
          if (current !== null) heroLeave(current, node);
          current = next;
          heroEnter(next, node, spec.heroMotion);
        });
      });
      onCleanup(() => {
        if (current !== null) heroLeave(current, node);
      });
    }
    if (spec.nudge) nudge(node, spec.nudge, spec.nudgeMotion);
    if (spec.nudgeKey) nudge(node, spec.nudgeKey, spec.nudgeMotion, spec.nudgeScale ?? 1.06, true);
    if (spec.scrollScale !== undefined || spec.scrollOpacity !== undefined) {
      scrollEffect(node, spec.scrollScale ?? 1, spec.scrollOpacity ?? 1, spec.scrollOriginX ?? 0.5, spec.scrollOriginY ?? 0.5);
    }
  }
  p.append(fragment);
}

// MARK: - Reflow

/// Children of `container` slide to new places with `motion` instead of
/// jumping there, whatever moved them: rows coming and going, a sibling
/// growing.
export function reflow(container, motion) {
  // Conditional branches and keyed rows build into a fragment. A comment
  // travels with that fragment and identifies its real layout parent once
  // the synchronous mounting pass has finished.
  const anchor = container.nodeType === Node.ELEMENT_NODE ? null : document.createComment("bern-reflow");
  if (anchor) container.append(anchor);
  let disposed = false;
  let release = null;
  const attach = (parent) => {
    if (disposed || !parent) return;
    let controller = parent.bernReflow;
    if (!controller) {
      const places = new Map();
      const observed = new Set();
      let stopped = false;
      const place = (child) => {
        const rect = layoutRect(child);
        const base = parent.getBoundingClientRect();
        return { left: rect.left - base.left, top: rect.top - base.top };
      };
      const measure = () => {
        places.clear();
        for (const child of parent.children) places.set(child, place(child));
      };
      const sizes = new ResizeObserver(() => play());
      const play = () => {
        if (stopped) return;
        const mounted = new Set(parent.children);
        for (const child of observed) {
          if (!mounted.has(child)) { sizes.unobserve(child); observed.delete(child); }
        }
        for (const child of mounted) {
          if (!observed.has(child)) { sizes.observe(child); observed.add(child); }
          if (child.bernExiting) continue;
          const before = places.get(child);
          if (!before) continue;
          const now = place(child);
          const dx = before.left - now.left;
          const dy = before.top - now.top;
          if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue;
          const layer = layers(child).layer("reflow");
          layer.x.set(layer.x.value + dx);
          layer.y.set(layer.y.value + dy);
          layer.x.to(0, motion);
          layer.y.to(0, motion);
        }
        measure();
      };
      const children = new MutationObserver(play);
      children.observe(parent, { childList: true });
      sizes.observe(parent);
      for (const child of parent.children) { sizes.observe(child); observed.add(child); }
      measure();
      const frame = requestAnimationFrame(() => { if (!stopped) measure(); });
      controller = parent.bernReflow = {
        users: 0,
        stop() {
          stopped = true;
          cancelAnimationFrame(frame);
          children.disconnect();
          sizes.disconnect();
          places.clear();
          observed.clear();
          if (parent.bernReflow === this) delete parent.bernReflow;
        },
      };
    }
    controller.users += 1;
    release = () => { if (--controller.users === 0) controller.stop(); };
  };
  if (anchor) queueMicrotask(() => attach(anchor.parentElement));
  else attach(container);
  onCleanup(() => {
    disposed = true;
    release?.();
    anchor?.remove();
  });
}

// MARK: - Nudges

/// A short nudge each time `when()` turns true: a little larger, then back
/// with `motion`; still with reduced motion.
export function nudge(node, when, motion, scale = 1.06, keyed = false) {
  let before = untrack(when);
  const layer = layers(node).layer("nudge");
  effect(() => {
    const now = when();
    if ((keyed ? !eq(now, before) : now && !before) && !reduced) {
      untrack(() => {
        layer.sx.to(scale, [0.12, 1], () => layer.sx.to(1, motion));
        layer.sy.to(scale, [0.12, 1], () => layer.sy.to(1, motion));
      });
    }
    before = now;
  });
}

// MARK: - Scroll effects

/// Scale and opacity by the distance of the element's center from the
/// middle of the scroll view it is in: at the middle `scale` and full
/// opacity; a half viewport plus half its size away, plain size and
/// `opacity`. Computed each frame the view scrolls or resizes.
function scrollEffect(node, scale, opacity, originX = 0.5, originY = 0.5) {
  if (reduced) return;
  const layer = layers(node).layer("scroll");
  let view = null;
  const update = () => {
    if (!view || !node.isConnected) return;
    const horizontal = view.classList.contains("bern-scroll-horizontal");
    const port = view.getBoundingClientRect();
    const rect = layoutRect(node);
    const reach = horizontal ? (port.width + rect.width) / 2 : (port.height + rect.height) / 2;
    const distance = horizontal
      ? rect.left + rect.width / 2 - (port.left + port.width / 2)
      : rect.top + rect.height / 2 - (port.top + port.height / 2);
    const near = reach > 0 ? 1 - Math.min(1, Math.abs(distance) / reach) : 1;
    const factor = 1 + (scale - 1) * near;
    layer.sx.set(factor);
    layer.sy.set(factor);
    // Elements scale about their center: a scale about another point is
    // that, moved by (1 − factor) × (point − center).
    layer.x.set((1 - factor) * (originX - 0.5) * rect.width);
    layer.y.set((1 - factor) * (originY - 0.5) * rect.height);
    layer.opacity.set(opacity + (1 - opacity) * near);
  };
  let pending = false;
  const schedule = () => {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => {
      pending = false;
      update();
    });
  };
  const observer = new ResizeObserver(schedule);
  // The scroll view is known once the element is on the page.
  queueMicrotask(() => {
    view = node.closest(".bern-scroll");
    if (!view) return;
    view.addEventListener("scroll", schedule, { passive: true });
    observer.observe(view);
    schedule();
  });
  onCleanup(() => {
    view?.removeEventListener("scroll", schedule);
    observer.disconnect();
  });
}

// MARK: - Idle windows

/// `window.idle` turns true after `seconds` without pointer, touch, wheel
/// or key input in the window, and false at the next input. With
/// `idle_area`s on the page, a mouse counts only while it rests over one,
/// for that area's seconds: elsewhere, or outside the window, the window
/// stays awake.
B.hooks.idle = (win, seconds, enabled = () => true) => {
  let timer = 0;
  // What the mouse is over, null once it left the window; touch and pens
  // leave it undefined, so areas do not apply to them.
  let mouseOver;
  const track = (event) => { mouseOver = event.pointerType === "mouse" ? event.target : undefined; };
  // How long the input that came last lets the window rest, or null when
  // it keeps the window awake.
  const quiet = () => {
    if (mouseOver === undefined || !win.element.querySelector(".bern-idle-area")) return seconds;
    const area = mouseOver?.closest?.(".bern-idle-area");
    return area ? Number(area.dataset.idleAfter) : null;
  };
  const rest = () => {
    clearTimeout(timer);
    if (win.idle.peek()) win.idle.set(false);
    const wait = quiet();
    if (enabled() && wait !== null) timer = setTimeout(() => {
      const typing = document.activeElement?.matches("input:not([type=range]),textarea");
      const popup = document.querySelector(".bern-popover:not([hidden]),.bern-menu-popover:not([hidden]),[popover]:popover-open");
      if (typing || popup) rest(); else win.idle.set(true);
    }, wait * 1000);
  };
  const events = ["pointermove", "pointerdown", "keydown", "wheel", "touchstart"];
  const leave = (event) => { if (event.pointerType === "mouse") { mouseOver = null; rest(); } };
  // The pointer is known before the input that wakes the window counts.
  for (const name of ["pointermove", "pointerdown"]) win.element.addEventListener(name, track, { capture: true, passive: true });
  for (const name of events) win.element.addEventListener(name, rest, { capture: true, passive: true });
  win.element.addEventListener("pointerleave", leave);
  effect(() => win.element.classList.toggle("bern-idle", win.idle.get()));
  let previousEnabled;
  effect(() => { const next = enabled(); if (next !== previousEnabled) { previousEnabled = next; rest(); } });
  onCleanup(() => {
    clearTimeout(timer);
    for (const name of ["pointermove", "pointerdown"]) win.element.removeEventListener(name, track, { capture: true });
    for (const name of events) win.element.removeEventListener(name, rest, { capture: true });
    win.element.removeEventListener("pointerleave", leave);
  });
  rest();
};

// MARK: - Files

/// Extensions of the file kinds `files:` names, from the build.
const KIND_EXTENSIONS = {};
const KIND_TYPES = { video: "video/", audio: "audio/", picture: "image/" };

export function fileKinds(table) {
  Object.assign(KIND_EXTENSIONS, table);
}

function words(kinds) {
  return (kinds ?? "any").split(/[\s,]+/).filter(Boolean).map((word) => word.toLowerCase());
}

function extensionOf(name) {
  const path = name.split(/[?#]/)[0];
  const dot = path.lastIndexOf(".");
  return dot > path.lastIndexOf("/") ? path.slice(dot + 1).toLowerCase() : "";
}

/// Whether a file named `name` (or at a URL), of MIME `type` when known,
/// is one of `kinds`.
export function accepts(name, type, kinds) {
  const ext = extensionOf(name ?? "");
  return words(kinds).some((word) => {
    if (word === "any") return true;
    if (word === "folder") return false;
    if (word.startsWith(".")) return ext === word.slice(1);
    if ((KIND_EXTENSIONS[word] ?? []).includes(ext)) return true;
    return Boolean(type && KIND_TYPES[word] && type.startsWith(KIND_TYPES[word]));
  });
}

/// The `accept` attribute of a file input for `kinds`.
function acceptOf(kinds) {
  const parts = [];
  for (const word of words(kinds)) {
    if (word === "any" || word === "folder") return "";
    if (word.startsWith(".")) parts.push(word);
    else {
      if (KIND_TYPES[word]) parts.push(`${KIND_TYPES[word]}*`);
      for (const ext of KIND_EXTENSIONS[word] ?? []) parts.push(`.${ext}`);
    }
  }
  return parts.join(",");
}

/// A page URL for a file the user gave: picked, dropped or opened with the
/// application. The browser keeps it while the page lives, and the engine
/// reads the file's name and bytes through it.
function fileURL(file) {
  const url = URL.createObjectURL(file);
  B.pageFiles.set(url, file);
  return url;
}

/// The browser's open panel for `kinds`; `give(url)` runs with the chosen
/// file, nothing on cancel.
export function pick(kinds, give) {
  const input = el("input", { type: "file", accept: acceptOf(kinds), hidden: true });
  if (words(kinds).includes("folder")) input.webkitdirectory = true;
  input.addEventListener("change", () => {
    const file = input.files?.[0];
    input.remove();
    if (file) give(fileURL(file));
  });
  input.addEventListener("cancel", () => input.remove());
  document.body.append(input);
  input.click();
}

/// Files the browser hands the page as an installed app's file handler
/// (`launchQueue`), for drop areas with `opens: true`.
const openers = [];
let launchConsumer = false;

function listenForLaunches() {
  if (launchConsumer || !("launchQueue" in window)) return;
  launchConsumer = true;
  window.launchQueue.setConsumer(async (params) => {
    for (const handle of params.files ?? []) {
      const file = await handle.getFile();
      const opener = [...openers].reverse().find((entry) => accepts(file.name, file.type, entry.kinds));
      opener?.give(fileURL(file));
    }
  });
}

// MARK: - Buttons with content

/// A button showing its content instead of a label: no frame of its own,
/// the content as it is; a style's corner clips it.
export function ContentButton(p, ctx, { id, label, enabled, style, shape, height, setFile, fileKinds: kinds, defaultFocus }, action, content) {
  // A capsule is the platform's button around the content, as it is around
  // a label; otherwise the content is all there is.
  const framed = shape === "capsule";
  const node = el("button", { class: framed ? "bern-button bern-button-standard" : "bern-content-button", type: "button" });
  B.bindAttr(node, "aria-label", label);
  content(node, ctx);
  const isEnabled = enabled ?? (() => true);
  effect(() => {
    node.disabled = !isEnabled();
  });
  B.mark(node, ctx, id);
  node.addEventListener("click", () => action?.());
  B.control(ctx, {
    target: () => B.target(ctx, id), role: "button", label, enabled: isEnabled,
    actions: { activate: () => action?.(), ...(setFile ? { setFile, fileKinds: kinds } : {}) },
  });
  if (style) applyStyle(node, style, {});
  // The web has no capsule buttons: a capsule is the standard button.
  if (shape === "capsule") node.classList.add("bern-control-capsule");
  else bindLength(node, "height", height);
  p.append(node);
  if (defaultFocus) B.startFocus(node);
}

// MARK: - Switches as buttons

/// `Switch(style: button)`, and any switch in a toolbar: a button that
/// stays pressed while on.
export function Switch(p, ctx, spec) {
  if (spec.style !== "button" && !ctx.inToolbar) {
    B.Switch(p, ctx, spec);
    return;
  }
  const { id, label, field, icon: glyph } = spec;
  const iconOnly = Boolean(glyph) && ctx.inToolbar;
  const node = el("button", {
    class: `${iconOnly ? "bern-icon-button" : "bern-button bern-button-standard"} bern-toggle-button`, type: "button",
  }, glyph ? B.icon(glyph, 16) : null);
  if (!iconOnly) {
    const text = el("span");
    B.bindText(text, label);
    node.append(text);
  }
  // A switch shown as a capsule or chip button is the web's pill chip.
  if (spec.shape === "capsule" || spec.shape === "chip") node.classList.add("bern-control-capsule");
  bindLength(node, "height", spec.height);
  B.bindAttr(node, "aria-label", label);
  B.bindAttr(node, "title", label);
  effect(() => node.setAttribute("aria-pressed", field.get() ? "true" : "false"));
  node.addEventListener("click", () => field.set(!field.peek()));
  B.mark(node, ctx, id);
  B.control(ctx, {
    target: () => B.target(ctx, id), role: "switch", label,
    actions: { setBool: (value) => field.set(value), activate: () => field.set(!field.peek()), value: () => (field.peek() ? "true" : "false") },
  });
  p.append(node);
}

// MARK: - Split buttons

/// A toggle linked with a button that shows `content` in a popover, such as
/// the usual settings of what the toggle turns on. External entries turn
/// it on or off with `set_bool` and open the popover with `activate`.
export function ControlGroup(p, ctx, { label, height }, content) {
  const node = el("div", { class: "bern-control-group", role: "group" });
  if (label) B.bindAttr(node, "aria-label", label);
  effect(() => node.style.setProperty("--bern-group-height", `${Math.max(24, Number((typeof height === "function" ? height() : height) ?? 40))}px`));
  content(node, ctx);
  p.append(node);
}

export function SplitButton(p, ctx, { id, label, icon: glyph, field, more, shape, height }, content) {
  const toggle = el("button", { class: "bern-icon-button bern-toggle-button bern-split-main", type: "button" },
    glyph ? B.icon(glyph, 16) : null);
  {
    const text = el("span");
    B.bindText(text, label);
    toggle.append(text);
  }
  B.bindAttr(toggle, "aria-label", label);
  B.bindAttr(toggle, "title", label);
  effect(() => toggle.setAttribute("aria-pressed", field.get() ? "true" : "false"));
  toggle.addEventListener("click", () => field.set(!field.peek()));
  const panelId = `bern-split-${Math.random().toString(36).slice(2)}`;
  const chevron = el("button", {
    class: "bern-icon-button bern-split-more", type: "button", "aria-haspopup": "dialog",
    "aria-expanded": "false", popovertarget: panelId,
  }, B.icon("chevron.down", 14));
  B.bindAttr(chevron, "aria-label", more ?? (() => B.tr("bern.split.more")));
  const panel = el("div", { class: "bern-popover-panel", id: panelId, popover: "auto", role: "dialog" });
  B.bindAttr(panel, "aria-label", label);
  content(panel, ctx);
  panel.addEventListener("beforetoggle", (event) => {
    const open = event.newState === "open";
    chevron.setAttribute("aria-expanded", open ? "true" : "false");
    if (open) {
      B.placePopover(panel, node);
      panel.style.minWidth = `${Math.max(240, node.getBoundingClientRect().width)}px`;
    }
  });
  onCleanup(() => {
    if (panel.matches(":popover-open")) panel.hidePopover();
  });
  const node = el("div", { class: "bern-split-button", role: "group" }, toggle, chevron, panel);
  // The web's split button is the standard one, at the standard height.
  if (shape === "capsule") {
    node.classList.add("bern-split-capsule");
    effect(() => node.classList.toggle("bern-split-selected", Boolean(field.get())));
  }
  B.mark(toggle, ctx, id);
  B.control(ctx, {
    target: () => B.target(ctx, id), role: "split_button", label,
    actions: {
      setBool: (value) => field.set(value),
      activate: () => panel.showPopover(),
      value: () => (field.peek() ? "true" : "false"),
    },
  });
  p.append(node);
}

// MARK: - Notices

/// A short notice that shows when it appears or `key` changes and hides
/// itself after `duration` seconds, read out politely. Notices of a window
/// share a place at its bottom edge.
export function Toast(p, ctx, { content, icon: glyph, key, duration, motion }) {
  const holder = ctx.win.element;
  let place = holder.querySelector(":scope > .bern-toasts");
  if (!place) {
    place = el("div", { class: "bern-toasts", role: "status", "aria-live": "polite" });
    holder.append(place);
  }
  let shown = null;
  let timer = 0;
  const hide = () => {
    const node = shown;
    shown = null;
    if (!node) return;
    const layer = layers(node).layer("enter");
    if (reduced) {
      node.remove();
      return;
    }
    layer.y.to(16, [0.25, 1]);
    layer.opacity.to(0, [0.25, 1], () => node.remove());
  };
  effect(() => {
    key();
    const text = content();
    untrack(() => {
      clearTimeout(timer);
      if (shown) shown.remove();
      const label = el("span");
      label.textContent = text;
      const node = el("div", { class: "bern-toast" }, glyph ? B.icon(glyph, 16) : null, label);
      place.append(node);
      shown = node;
      const layer = layers(node).layer("enter");
      layer.y.set(24);
      layer.opacity.set(0);
      layer.y.to(0, motion);
      layer.opacity.to(1, motion);
      timer = setTimeout(hide, (duration ?? 3) * 1000);
    });
  });
  onCleanup(() => {
    clearTimeout(timer);
    shown?.remove();
  });
}

// MARK: - Inspector

/// A side panel at the trailing edge of the detail column that pushes the
/// content aside while `presented`; on narrow windows a sheet from the
/// bottom. It slides in and out with `motion`.
export function Inspector(p, ctx, { presented, width, motion }, children) {
  const panel = el("aside", { class: "bern-inspector" });
  const size = width ?? 280;
  panel.style.setProperty("--bern-inspector-width", `${size}px`);
  children(panel, ctx);
  p.append(panel);
  let column = null;
  const layer = layers(panel).layer("enter");
  const narrow = () => panel.ownerDocument.defaultView.innerWidth < 768;
  let first = true;
  effect(() => {
    const open = presented.get();
    untrack(() => {
      column ??= panel.closest(".bern-detail");
      column?.classList.toggle("bern-inspecting", open);
      column?.style.setProperty("--bern-inspector-width", `${size}px`);
      panel.classList.toggle("bern-inspector-open", open);
      const away = narrow() ? { x: 0, y: 320 } : { x: size, y: 0 };
      if (first) {
        first = false;
        layer.x.set(open ? 0 : away.x);
        layer.y.set(open ? 0 : away.y);
        panel.hidden = !open;
        return;
      }
      if (open) {
        panel.hidden = false;
        layer.x.set(layer.x.value || away.x);
        layer.y.set(layer.y.value || away.y);
        layer.x.to(0, motion);
        layer.y.to(0, motion);
      } else {
        let left = 2;
        const done = () => {
          if (--left === 0 && !presented.peek()) panel.hidden = true;
        };
        layer.x.to(away.x, motion, done);
        layer.y.to(away.y, motion, done);
      }
    });
  });
  // On narrow windows Escape and the back gesture close the sheet.
  panel.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      presented.set(false);
    }
  });
  onCleanup(() => column?.classList.remove("bern-inspecting"));
}

// MARK: - Drop areas

/// Its content accepts a file dragged onto it: while an acceptable file is
/// over it `over` is true; dropping stores the file in `field` and runs the
/// action. Other files are refused. With `opens`, files the browser hands
/// the installed page also arrive here.
export function Drop(p, ctx, { id, label, field, kinds, over, opens }, action, content) {
  const node = el("div", { class: "bern-drop" });
  B.bindAttr(node, "aria-label", label);
  content(node, ctx);
  const give = (url) => {
    field.set({ url });
    action?.();
  };
  const acceptable = (event) => {
    const items = [...(event.dataTransfer?.items ?? [])].filter((item) => item.kind === "file");
    if (!items.length) return false;
    // While dragging, browsers show types but not names.
    return items.some((item) => !item.type || accepts("", item.type, kinds) || words(kinds).includes("any"));
  };
  let depth = 0;
  node.addEventListener("dragenter", (event) => {
    if (!acceptable(event)) return;
    event.preventDefault();
    if (depth++ === 0) over?.set(true);
  });
  node.addEventListener("dragover", (event) => {
    if (!acceptable(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  });
  node.addEventListener("dragleave", () => {
    if (depth > 0 && --depth === 0) over?.set(false);
  });
  node.addEventListener("drop", (event) => {
    event.preventDefault();
    depth = 0;
    over?.set(false);
    const file = [...(event.dataTransfer?.files ?? [])].find((one) => accepts(one.name, one.type, kinds));
    if (file) give(fileURL(file));
  });
  if (opens) {
    const entry = { kinds, give };
    openers.push(entry);
    onCleanup(() => openers.splice(openers.indexOf(entry), 1));
    listenForLaunches();
  }
  B.mark(node, ctx, id);
  B.control(ctx, {
    target: () => B.target(ctx, id), role: "drop", label,
    actions: { setFile: give, fileKinds: kinds ?? "any", value: () => field.peek()?.url ?? null },
  });
  p.append(node);
}

// MARK: - Gestures bound to values

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

/// Pinching the content — two fingers, or a trackpad pinch, which the
/// browser reports as a wheel with Control — scales `field` within `min`
/// and `max`; `commit` runs when the gesture ends.
export function Pinch(p, ctx, { id, label, field, min, max }, commit, content) {
  const node = el("div", { class: "bern-gesture bern-pinch" });
  content(node, ctx);
  let settle = 0;
  node.addEventListener("wheel", (event) => {
    if (!event.ctrlKey) return;
    event.preventDefault();
    field.set(clamp(field.peek() * Math.exp(-event.deltaY * 0.01), min, max));
    clearTimeout(settle);
    settle = setTimeout(() => commit?.(), 200);
  }, { passive: false });
  const touches = new Map();
  let start = null;
  const spread = () => {
    const [a, b] = [...touches.values()];
    return Math.hypot(a.x - b.x, a.y - b.y);
  };
  node.addEventListener("pointerdown", (event) => {
    if (event.pointerType !== "touch") return;
    touches.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (touches.size === 2) start = { distance: spread(), value: field.peek() };
  });
  node.addEventListener("pointermove", (event) => {
    if (!touches.has(event.pointerId)) return;
    touches.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (touches.size === 2 && start?.distance > 0) {
      field.set(clamp(start.value * (spread() / start.distance), min, max));
    }
  });
  const end = (event) => {
    if (!touches.delete(event.pointerId)) return;
    if (start && touches.size < 2) {
      start = null;
      commit?.();
    }
  };
  for (const name of ["pointerup", "pointercancel"]) node.addEventListener(name, end);
  B.mark(node, ctx, id);
  B.control(ctx, {
    target: () => B.target(ctx, id), role: "pinch", label,
    actions: {
      setNumber: (value) => {
        field.set(clamp(value, min, max));
        commit?.();
      },
      value: () => B.num(field.peek()),
    },
  });
  p.append(node);
}

/// Dragging over the content along `axis` moves `field` through its range
/// over `distance` pixels (up and right increase it); `commit` runs when
/// the pointer lifts.
export function Swipe(p, ctx, { id, label, field, min, max, axis, distance }, commit, content) {
  const vertical = (axis ?? "vertical") === "vertical";
  const node = el("div", { class: `bern-gesture bern-swipe bern-swipe-${vertical ? "vertical" : "horizontal"}` });
  content(node, ctx);
  const span = max - min;
  const reach = distance ?? 200;
  let drag = null;
  node.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    drag = { x: event.clientX, y: event.clientY, from: field.peek(), moving: false, pointer: event.pointerId };
  });
  node.addEventListener("pointermove", (event) => {
    if (!drag || event.pointerId !== drag.pointer) return;
    const moved = vertical ? drag.y - event.clientY : event.clientX - drag.x;
    if (!drag.moving && Math.abs(moved) < 10) return;
    if (!drag.moving) {
      drag.moving = true;
      node.setPointerCapture(event.pointerId);
    }
    field.set(clamp(drag.from + (moved / reach) * span, min, max));
  });
  const end = () => {
    if (drag?.moving) commit?.();
    drag = null;
  };
  for (const name of ["pointerup", "pointercancel"]) node.addEventListener(name, end);
  B.mark(node, ctx, id);
  B.control(ctx, {
    target: () => B.target(ctx, id), role: "swipe", label,
    actions: {
      setNumber: (value) => {
        field.set(clamp(value, min, max));
        commit?.();
      },
      value: () => B.num(field.peek()),
    },
  });
  p.append(node);
}

// MARK: - Scrolling to items

/// A scroll view whose scrolling comes to rest with an item at the start
/// or center (`snap`), whose `position` field follows the item there and
/// scrolls to the item it is set to, giving it a nudge on arrival with
/// `arrival: "nudge"`. Items are the rows of the lists inside, each known
/// by its key.
export function ScrollView(p, ctx, { axis, snap, position, arrival, nudgeMotion, hideIndicators, openAt }, children) {
  const horizontal = axis === "horizontal";
  const node = el("div", { class: `bern-scroll bern-scroll-${axis ?? "vertical"} bern-fill-width bern-fill-height` });
  if (hideIndicators) node.classList.add("bern-scroll-indicators-hidden");
  if (snap) node.classList.add("bern-snap", `bern-snap-${snap}`);
  children(node, ctx);
  p.append(node);
  if (openAt !== undefined) B.openScroll(node, horizontal, openAt);
  if (!snap && !position) return;
  // A position binding follows the leading item unless center snapping
  // was explicitly requested (for example by a horizontal episode rail).
  const anchor = snap ?? "start";
  /// Rows of this scroll view's lists, not of nested scroll views.
  const items = () => [...node.querySelectorAll("[data-bern-item]")]
    .filter((item) => item.closest(".bern-scroll") === node);
  const along = (rect) => (horizontal ? rect.left : rect.top);
  const extent = (rect) => (horizontal ? rect.width : rect.height);
  /// The scroll offset that puts `item` at the anchor.
  const offsetOf = (item) => {
    const port = node.getBoundingClientRect();
    const rect = layoutRect(item);
    const start = along(rect) - along(port) + (horizontal ? node.scrollLeft : node.scrollTop);
    if (anchor === "start") return start - (horizontal ? parseFloat(getComputedStyle(node).paddingLeft) : parseFloat(getComputedStyle(node).paddingTop));
    return start + extent(rect) / 2 - extent(port) / 2;
  };
  const current = () => (horizontal ? node.scrollLeft : node.scrollTop);
  const nearest = () => {
    let best = null;
    let distance = Infinity;
    for (const item of items()) {
      const gap = Math.abs(offsetOf(item) - current());
      if (gap < distance) {
        distance = gap;
        best = item;
      }
    }
    return best;
  };
  // Room before the first and after the last item, so they can come to
  // the center too.
  const margins = () => {
    if (anchor !== "center") return;
    const property = horizontal ? "paddingInline" : "paddingBlock";
    // Measure without the centering space from the last pass. In an
    // intrinsically sized flex child that space increases the viewport's
    // own size; feeding it back into this calculation grows it forever.
    node.style[property] = "0px";
    const viewport = horizontal ? node.clientWidth : node.clientHeight;
    const rows = items();
    const largest = Math.max(0, ...rows.map((item) => extent(layoutRect(item))));
    const room = rows.length ? Math.max(0, (viewport - largest) / 2) : 0;
    node.style[property] = `${room}px`;
  };
  const resize = new ResizeObserver(margins);
  resize.observe(node);
  const tagged = new MutationObserver(() => {
    for (const item of items()) resize.observe(item);
    margins();
  });
  tagged.observe(node, { childList: true, subtree: true });
  for (const item of items()) resize.observe(item);
  onCleanup(() => {
    resize.disconnect();
    tagged.disconnect();
  });
  if (!position) return;
  let placed = false;
  let travelling = null;
  const scrollTo = (item, smooth) => {
    const offset = offsetOf(item);
    node.scrollTo({ [horizontal ? "left" : "top"]: offset, behavior: smooth && !reduced ? "smooth" : "instant" });
  };
  // The field follows the item the user leaves at the anchor.
  let resting = 0;
  const rest = () => {
    clearTimeout(resting);
    resting = setTimeout(() => {
      if (!placed) return;
      const item = nearest();
      if (travelling) {
        // A scroll the field asked for has arrived.
        const arrived = travelling;
        travelling = null;
        if (arrival === "nudge" && item === arrived) nudgeOnce(arrived, nudgeMotion);
        return;
      }
      if (item && !eq(item.bernKey, position.peek())) position.set(item.bernKey);
    }, 140);
  };
  node.addEventListener("scroll", rest, { passive: true });
  onCleanup(() => clearTimeout(resting));
  // Setting the field scrolls to its item.
  effect(() => {
    const key = position.get();
    untrack(() => {
      if (!placed) return;
      const item = items().find((one) => eq(one.bernKey, key));
      if (!item || item === nearest() && Math.abs(offsetOf(item) - current()) < 1) return;
      travelling = item;
      scrollTo(item, true);
    });
  });
  // The first display starts at the field's item, without a nudge.
  requestAnimationFrame(() => requestAnimationFrame(() => {
    margins();
    const item = items().find((one) => eq(one.bernKey, position.peek()));
    if (item) scrollTo(item, false);
    placed = true;
  }));
}

function nudgeOnce(node, motion) {
  if (reduced) return;
  const layer = layers(node).layer("nudge");
  layer.sx.to(1.06, [0.12, 1], () => layer.sx.to(1, motion));
  layer.sy.to(1.06, [0.12, 1], () => layer.sy.to(1, motion));
}

// MARK: - Pictures

/// A picture from a declared resource or from a `MediaSource` the engine
/// gives (a file the user chose, or an http(s) URL); with one side given,
/// the other follows the picture.
export function Image(p, ctx, { source, url, label, width, height, fit, animate, corner, rendering, aspect, adaptive }) {
  const template = rendering === "template";
  const intrinsicRatio = cell(aspect ?? 1);
  const probe = template && !aspect ? new globalThis.Image() : null;
  if (probe) {
    probe.onload = () => {
      if (probe.naturalHeight) intrinsicRatio.set(probe.naturalWidth / probe.naturalHeight);
    };
    onCleanup(() => { probe.onload = null; });
  }
  const node = template ? el("span", { class: "bern-image bern-image-template" })
    : el("img", { class: "bern-image", alt: "", draggable: "false", decoding: "async" });
  node.style.objectFit = fit ?? "contain";
  const setSource = (value) => {
    if (template) {
      node.style.maskImage = value ? `url(${JSON.stringify(value)})` : "none";
      node.style.webkitMaskImage = node.style.maskImage;
      node.style.maskSize = fit === "cover" ? "cover" : fit === "fill" ? "100% 100%" : "contain";
      if (probe && value) probe.src = value;
    } else if (value) node.src = value;
    else node.removeAttribute("src");
  };
  if (source) setSource(resource(source));
  if (url) {
    effect(() => {
      const value = url();
      setSource(value);
      node.hidden = !value;
    });
  }
  bindLength(node, "width", width, animate);
  bindLength(node, "height", height, animate);
  if (template && width === undefined && height !== undefined) {
    bindLength(node, "width", () => Number(typeof height === "function" ? height() : height) * intrinsicRatio.get(), animate);
  } else if (template && height === undefined && width !== undefined) {
    bindLength(node, "height", () => Number(typeof width === "function" ? width() : width) / intrinsicRatio.get(), animate);
  }
  bindLength(node, "borderRadius", corner, animate);
  if (aspect) node.style.aspectRatio = String(aspect);
  if (adaptive || (aspect && width === undefined)) {
    node.classList.add("bern-fill-width");
    node.style.width = "100%";
    if (height === undefined) node.style.height = "auto";
  }
  if (template) {
    node.setAttribute("role", "img");
    if (label) B.bindAttr(node, "aria-label", label);
    else node.setAttribute("aria-hidden", "true");
  } else if (label) B.bindAttr(node, "alt", label);
  p.append(ctx.inGroup ? el("div", { class: "bern-row bern-row-media" }, node) : node);
}

// MARK: - Theme chosen at run time

/// `WindowSet(accent:, appearance:)`: the accent named by `accent()` and the
/// appearance (`light`, `dark`, or the system's) for every window.
export function theme(accent, appearance) {
  const root = document.documentElement;
  effect(() => {
    const name = accent?.();
    if (name) root.dataset.bernAccent = name.toLowerCase();
    else delete root.dataset.bernAccent;
    const mode = appearance?.()?.toLowerCase();
    if (mode === "light" || mode === "dark") root.dataset.bernAppearance = mode;
    else delete root.dataset.bernAppearance;
    untrack(() => themeTick.set(themeTick.peek() + 1));
  });
}

// MARK: - Shortcuts

/// Keyboard shortcuts of the controls on the page: `cmd` is ⌘ on Apple
/// systems and Control elsewhere. The latest control wins; a key without a
/// modifier does not fire while text is being edited.
const shortcuts = [];
const apple = /Mac|iPhone|iPad/.test(navigator.platform);
const KEY_NAMES = {
  " ": "space", Escape: "escape", Enter: "return", Tab: "tab", Backspace: "delete", Delete: "delete",
  ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Home: "home", End: "end",
  PageUp: "pageup", PageDown: "pagedown",
};

function pressed(event) {
  const key = KEY_NAMES[event.key] ?? (event.code.startsWith("Key") ? event.code.slice(3).toLowerCase()
    : event.code.startsWith("Digit") ? event.code.slice(5) : event.key.toLowerCase());
  const parts = [];
  if (apple ? event.metaKey : event.ctrlKey) parts.push("cmd");
  if (apple && event.ctrlKey) parts.push("ctrl");
  if (event.altKey) parts.push("alt");
  if (event.shiftKey) parts.push("shift");
  parts.push(key);
  return parts.join("+");
}

function normalized(shortcut) {
  const parts = shortcut.toLowerCase().split("+");
  const key = parts.pop();
  const order = ["cmd", "ctrl", "alt", "shift"];
  const mods = order.filter((mod) => parts.includes(mod) || (mod === "cmd" && !apple && parts.includes("ctrl")));
  return [...new Set(mods), key].join("+");
}

let listening = false;

/// `spec` runs `run` while it is mounted; `active`, when given, says
/// whether it takes the key now. Of the shortcuts that take a key, the one
/// mounted last runs.
export function shortcut(spec, run, active) {
  const entry = { keys: normalized(spec), run, active };
  shortcuts.push(entry);
  onCleanup(() => shortcuts.splice(shortcuts.indexOf(entry), 1));
  if (listening) return;
  listening = true;
  document.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    const keys = pressed(event);
    const found = [...shortcuts].reverse().find((one) => one.keys === keys && (!one.active || one.active()));
    if (!found) return;
    const editing = event.target.closest?.("input, textarea, [contenteditable]");
    if (editing && !/^(cmd|ctrl|alt)\+/.test(keys)) return;
    event.preventDefault();
    found.run();
  }, true);
}

/// `Shortcut`: `key` runs `action` while it is mounted and enabled, without
/// a control on the page.
export function KeyShortcut(p, ctx, { key, enabled }, action) {
  if (!action) return;
  const active = enabled === undefined ? undefined
    : () => Boolean(typeof enabled === "function" ? untrack(enabled) : enabled);
  shortcut(key, () => untrack(action), active);
}

/// Focus the control at `target`, as `ui.focus` does: search fields only.
export function focusTarget(ctx, id) {
  const entry = B.controlEntries().find((one) => one.target() === B.target(ctx, id));
  entry?.actions.focus?.();
}

// MARK: - Rails

/// A strip along `axis` that places each child at a fraction of its length
/// (`at`) or between two (`from`, `to`). Across a horizontal rail the
/// children share one row, so the rail is as thick as the thickest; a
/// vertical rail takes the height it is given. The rail takes no clicks;
/// its children do.
export function Rail(p, ctx, { axis, anchor, overhang }, children) {
  const vertical = axis === "vertical";
  const node = el("div", { class: `bern-rail bern-rail-${vertical ? "vertical" : "horizontal"} bern-fill-width` });
  if (vertical) node.classList.add("bern-fill-height");
  // The scale runs this far past both ends (the stylesheet's calc).
  bindNumber(overhang, null, (value) => node.style.setProperty("--bern-rail-overhang", `${value}px`));
  children(node, { ...ctx, rail: { vertical, anchor: anchor ?? "center" } });
  p.append(node);
}

const ANCHOR_SHIFT = { start: "0%", center: "-50%", end: "-100%" };

/// Places the element `build` makes on the rail it stands in. Fractions
/// outside 0…1 stop at the ends; with `motion` the element springs to a
/// new place.
export function Place(p, ctx, { at, from, to, motion }, build) {
  const fragment = document.createDocumentFragment();
  build(fragment, ctx);
  const rail = ctx.rail ?? { vertical: false, anchor: "center" };
  const clampFraction = (value) => Math.min(1, Math.max(0, Number(value) || 0));
  for (const node of fragment.children) {
    node.classList.add("bern-rail-item");
    if (at !== undefined) {
      const shift = ANCHOR_SHIFT[rail.anchor] ?? "-50%";
      node.style.translate = rail.vertical ? `0 ${shift}` : `${shift} 0`;
      bindNumber(at, motion, (value) => {
        node.style.setProperty("--bern-rail-at", String(clampFraction(value)));
      });
    } else {
      node.classList.add("bern-rail-span");
      const ends = { from: 0, to: 0 };
      const write = () => {
        const a = clampFraction(ends.from);
        const b = clampFraction(ends.to);
        node.style.setProperty("--bern-rail-at", String(Math.min(a, b)));
        node.style.setProperty("--bern-rail-span", String(Math.abs(b - a)));
      };
      bindNumber(from, motion, (value) => {
        ends.from = value;
        write();
      });
      bindNumber(to, motion, (value) => {
        ends.to = value;
        write();
      });
    }
  }
  p.append(fragment);
}

// MARK: - Popover buttons

/// A button that shows its children in a popover, such as a volume slider.
/// The children mount while the popover shows. `presented`, when given,
/// follows the popover and opens or closes it; external entries open it
/// with `activate`.
export function PopoverButton(p, ctx, { id, label, icon: glyph, presented }, content) {
  const panelId = `bern-popover-${Math.random().toString(36).slice(2)}`;
  const iconOnly = Boolean(glyph);
  const trigger = el("button", {
    class: iconOnly ? "bern-icon-button bern-popover-button" : "bern-button bern-button-standard bern-popover-button",
    type: "button", "aria-haspopup": "dialog", "aria-expanded": "false", popovertarget: panelId,
  }, glyph ? B.icon(glyph, 16) : null);
  if (!iconOnly) {
    const text = el("span");
    B.bindText(text, label);
    trigger.append(text);
  }
  B.bindAttr(trigger, "aria-label", label);
  B.bindAttr(trigger, "title", label);
  const panel = el("div", { class: "bern-popover-panel", id: panelId, popover: "auto", role: "dialog" });
  B.bindAttr(panel, "aria-label", label);
  const open = cell(false);
  let owner = null;
  panel.addEventListener("beforetoggle", (event) => {
    const showing = event.newState === "open";
    trigger.setAttribute("aria-expanded", showing ? "true" : "false");
    if (showing) {
      // The children mount with the popover and leave with it.
      owner = new B.Scope(null);
      B.within(owner, () => content(panel, ctx));
      B.placePopover(panel, trigger);
      panel.style.minWidth = `${Math.max(240, trigger.getBoundingClientRect().width)}px`;
    }
  });
  panel.addEventListener("toggle", (event) => {
    const showing = event.newState === "open";
    if (!showing) {
      owner?.dispose();
      owner = null;
      panel.replaceChildren();
    }
    open.set(showing);
    if (presented && presented.peek() !== showing) presented.set(showing);
  });
  if (presented) {
    const follow = () => {
      const want = presented.peek();
      const showing = panel.matches(":popover-open");
      if (want && !showing && panel.isConnected) panel.showPopover();
      else if (!want && showing) panel.hidePopover();
    };
    effect(() => {
      presented.get();
      untrack(follow);
    });
    // Presented from the start: open once the button is on the page.
    queueMicrotask(follow);
  }
  onCleanup(() => {
    if (panel.matches(":popover-open")) panel.hidePopover();
    owner?.dispose();
  });
  const node = el("div", { class: "bern-popover-wrap" }, trigger, panel);
  B.mark(trigger, ctx, id);
  B.control(ctx, {
    target: () => B.target(ctx, id), role: "popover_button", label,
    actions: {
      activate: () => panel.showPopover(),
      value: () => (open.peek() ? "open" : "closed"),
    },
  });
  p.append(node);
}

/// Runs the `activate` of the control at `id`, as `ui.activate` does.
export function activateTarget(ctx, id) {
  const entry = B.controlEntries().find((one) => one.target() === B.target(ctx, id));
  entry?.actions.activate?.();
}

export function bindPosition(node, property, value, motion) {
  bindNumber(value, motion, (v) => { node.style[property] = `${v}px`; });
}

/// A `Positioned` layer: its place and size in the scene, each on the
/// layer's motion. With `bleed`, each side that reaches the edge of the
/// scene continues past it, to the edge of the room the `ScaleBox` has
/// around the scene, so the layer meets the window's edges whatever the
/// window's shape. The sides follow the place the layer is going to, on
/// the same motion; a change of the window's shape moves them at once.
export function bindLayer(node, _ctx, { x, y, width, height, bleed }, motion) {
  node.classList.add("bern-sized-width", "bern-sized-height");
  const at = { x: 0, y: 0, width: 0, height: 0 };
  const out = { left: 0, right: 0, top: 0, bottom: 0 };
  const write = () => {
    node.style.left = `${at.x - out.left}px`;
    node.style.top = `${at.y - out.top}px`;
    node.style.width = `${Math.max(0, at.width + out.left + out.right)}px`;
    node.style.height = `${Math.max(0, at.height + out.top + out.bottom)}px`;
  };
  for (const [key, value] of [["x", x], ["y", y], ["width", width], ["height", height]]) {
    bindNumber(value, motion, (v) => { at[key] = v; write(); });
  }
  const room = node.bernScene;
  if (bleed === undefined || bleed === null || bleed === false || !room) return;
  const springs = Object.fromEntries(Object.keys(out).map((side) =>
    [side, new Spring(0, (v) => { out[side] = Math.max(0, v); write(); })]));
  const read = (value) => Number(typeof value === "function" ? value() : value) || 0;
  let last = null;
  effect(() => {
    const scene = room.get();
    const on = !!(typeof bleed === "function" ? bleed() : bleed);
    const left = read(x), top = read(y), w = read(width), h = read(height);
    // Half a point of slack: a layer laid out to the edge reaches it.
    const target = {
      left: on && left <= 0.5 ? scene.x : 0,
      right: on && left + w >= scene.width - 0.5 ? scene.x : 0,
      top: on && top <= 0.5 ? scene.y : 0,
      bottom: on && top + h >= scene.height - 0.5 ? scene.y : 0,
    };
    const jump = last === null || last.x !== scene.x || last.y !== scene.y;
    last = scene;
    untrack(() => {
      for (const side of Object.keys(target)) {
        if (jump) springs[side].set(target[side]);
        else springs[side].to(target[side], motion);
      }
    });
  });
}
