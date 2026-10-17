// Added only when the application declares Svg. The opaque-origin iframe
// runs SVG scripts without access to the application's DOM or engine.
const bernSvgPrefix = /*bern:svg-prefix*/;
const bernSvgSuffix = /*bern:svg-suffix*/;
export function Svg(p, ctx, { source, label, width, height, fit = "contain", playing, speed, restart, interactive = true }) {
  const frame = el("iframe", { sandbox: "allow-scripts", class: "bern-svg", scrolling: "no", referrerpolicy: "no-referrer" });
  frame.style.border = "0";
  frame.style.background = "transparent";
  frame.style.display = "block";
  frame.style.flexShrink = "0";
  if (label) B.bindAttr(frame, "title", label);
  else { frame.setAttribute("aria-hidden", "true"); frame.tabIndex = -1; }
  bindLength(frame, "width", width);
  bindLength(frame, "height", height);
  let loaded = false, visible = true, current = {}, disposed = false;
  const read = (value, fallback) => typeof value === "function" ? value() : value ?? fallback;
  const send = () => {
    if (loaded) frame.contentWindow?.postMessage({ ...current, type: "bern:svg", visible }, "*");
  };
  frame.addEventListener("load", () => { loaded = true; send(); });
  const observer = new IntersectionObserver(entries => { visible = entries[0]?.isIntersecting ?? true; send(); });
  observer.observe(frame);
  effect(() => {
    current = { playing: read(playing, true), speed: Number(read(speed, 1)), restart: Number(read(restart, 0)), fit, interactive };
    send();
  });
  const abort = new AbortController();
  fetch(resource(source), { signal: abort.signal }).then(response => {
    if (!response.ok) throw new Error(`SVG ${source}: ${response.status}`);
    return response.text();
  }).then(source => {
    if (!disposed) frame.srcdoc = bernSvgPrefix + source + bernSvgSuffix;
  }).catch(error => { if (error.name !== "AbortError") console.error(error); });
  onCleanup(() => {
    disposed = true;
    abort.abort();
    observer.disconnect();
    frame.srcdoc = "";
  });
  p.append(frame);
}
