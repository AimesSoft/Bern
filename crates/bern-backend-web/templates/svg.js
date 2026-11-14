// Added only when the application declares `Svg` or `WebPage`.
// `Svg` renders precompiled `BSVG` scenes through the pure-Rust `bern-svg`
// player in `app.wasm` onto a `<canvas>` — no `<iframe>` or DOM subdocument.
export function Svg(p, ctx, { source, label, width, height, fit = "contain", playing, speed, restart, interactive = true }) {
  const canvas = el("canvas", { class: "bern-svg" });
  canvas.style.display = "block";
  canvas.style.flexShrink = "0";
  if (label) {
    canvas.setAttribute("role", "img");
    B.bindAttr(canvas, "aria-label", label);
  } else {
    canvas.setAttribute("aria-hidden", "true");
  }
  bindLength(canvas, "width", width);
  bindLength(canvas, "height", height);
  const ctx2d = canvas.getContext("2d");

  let player = 0, visible = true, disposed = false, raf = 0, active = true;
  const fitCode = fit === "cover" ? 1 : (fit === "fill" ? 2 : 0);
  const read = (value, fallback) => typeof value === "function" ? value() : value ?? fallback;
  let cfgPlaying = true, cfgSpeed = 1, cfgRestart = 0;

  function stopLoop() {
    if (raf) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
  }

  function renderFrame(nowMs) {
    if (disposed || !player || !ctx2d) return;
    const rect = canvas.getBoundingClientRect();
    const cssW = Math.max(1, rect.width || parseFloat(canvas.style.width) || 1);
    const cssH = Math.max(1, rect.height || parseFloat(canvas.style.height) || 1);
    const dpr = Math.max(1, window.devicePixelRatio || 1);
    const wPx = Math.max(1, Math.round(cssW * dpr));
    const hPx = Math.max(1, Math.round(cssH * dpr));
    if (canvas.width !== wPx || canvas.height !== hPx) {
      canvas.width = wPx;
      canvas.height = hPx;
    }
    const isDark = dark(canvas);
    const textRgba = isDark ? 0xFFFFFFFF : 0x000000FF;
    const runNow = cfgPlaying && visible && !document.hidden;
    B.svgConfigure(player, runNow, cfgSpeed, cfgRestart, fitCode, interactive, isDark, reduceMotion(), textRgba);
    const { status, pixels } = B.svgRender(player, nowMs * 0.001, wPx, hPx);
    if ((status & 1) !== 0 && pixels) {
      ctx2d.putImageData(new ImageData(pixels, wPx, hPx), 0, 0);
    }
    active = (status & 2) !== 0;
    syncLoop();
  }

  function tick(nowMs) {
    raf = 0;
    renderFrame(nowMs);
  }

  function syncLoop() {
    const want = !disposed && player && active && cfgPlaying && visible && !document.hidden;
    if (want && !raf) raf = requestAnimationFrame(tick);
    else if (!want && raf) stopLoop();
  }

  const io = new IntersectionObserver(entries => {
    visible = entries[0]?.isIntersecting ?? true;
    if (visible) renderFrame(performance.now());
    else syncLoop();
  });
  io.observe(canvas);

  const ro = new ResizeObserver(() => renderFrame(performance.now()));
  ro.observe(canvas);

  const onVis = () => {
    if (!document.hidden) renderFrame(performance.now());
    else syncLoop();
  };
  document.addEventListener("visibilitychange", onVis);

  if (interactive) {
    const norm = (ev) => {
      const r = canvas.getBoundingClientRect();
      return [
        r.width > 0 ? (ev.clientX - r.left) / r.width : 0.5,
        r.height > 0 ? (ev.clientY - r.top) / r.height : 0.5,
      ];
    };
    const sendPtr = (kind, ev) => {
      if (!player) return;
      const [nx, ny] = ev ? norm(ev) : [0, 0];
      const st = B.svgPointer(player, kind, nx, ny);
      if ((st & 2) !== 0) active = true;
      if ((st & 1) !== 0 || (st & 2) !== 0) renderFrame(performance.now());
    };
    canvas.addEventListener("click", ev => sendPtr(0, ev));
    canvas.addEventListener("dblclick", ev => { ev.preventDefault(); sendPtr(1, ev); });
    canvas.addEventListener("pointermove", ev => sendPtr(2, ev));
    canvas.addEventListener("pointerleave", () => sendPtr(3, null));
    canvas.addEventListener("pointerdown", ev => sendPtr(4, ev));
    canvas.addEventListener("pointerup", ev => sendPtr(5, ev));
  }

  effect(() => {
    themeTick.get();
    cfgPlaying = Boolean(read(playing, true));
    const sp = Number(read(speed, 1));
    cfgSpeed = Number.isFinite(sp) && sp > 0 ? sp : 1;
    const rs = Number(read(restart, 0));
    cfgRestart = Number.isFinite(rs) ? rs : 0;
    if (player) renderFrame(performance.now());
  });

  const abort = new AbortController();
  if (B.svgCached(source)) {
    player = B.svgOpen(source, null);
    renderFrame(performance.now());
  } else {
    fetch(resource(source), { signal: abort.signal })
      .then(response => {
        if (!response.ok) throw new Error(`SVG ${source}: ${response.status}`);
        return response.arrayBuffer();
      })
      .then(buf => {
        if (disposed) return;
        player = B.svgOpen(source, new Uint8Array(buf));
        renderFrame(performance.now());
      })
      .catch(error => { if (error.name !== "AbortError") console.error(error); });
  }

  onCleanup(() => {
    disposed = true;
    stopLoop();
    abort.abort();
    io.disconnect();
    ro.disconnect();
    document.removeEventListener("visibilitychange", onVis);
    if (player) {
      B.svgFree(player);
      player = 0;
    }
  });

  p.append(canvas);
}

// `WebPage`: another site's page in a frame. Its cookies are that site's own:
// a page cannot set them for it.
export function WebPage(p, ctx, { url, label }) {
  const frame = el("iframe", { referrerpolicy: "no-referrer" });
  frame.style.border = "0";
  frame.style.width = "100%";
  frame.style.height = "100%";
  frame.style.flex = "1";
  if (url) B.bindAttr(frame, "src", url);
  if (label) B.bindAttr(frame, "title", label);
  p.append(frame);
}
