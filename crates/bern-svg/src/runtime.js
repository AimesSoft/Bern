// One document-local runtime for WebKit, WebView2, Android and the Web.
// No native bridge. Only the parent may send animation controls.
(() => {
  "use strict";
  const svg = document.querySelector("body > svg");
  if (!svg) return;
  let playing = true, visible = true, speed = 1, revision = null, externalLinks = false;
  let frame = 0, previous = 0;
  const owned = new WeakSet();
  const active = () => playing && visible && !document.hidden;
  const animations = () => document.getAnimations ? document.getAnimations() : [];
  function sync() {
    for (const animation of animations()) {
      animation.playbackRate = speed;
      if (!active()) {
        if (animation.playState === "running" || animation.pending) {
          owned.add(animation);
          animation.pause();
        }
      } else if (owned.has(animation)) {
        owned.delete(animation);
        animation.play();
      }
    }
    if (svg.pauseAnimations) {
      if (active() && speed === 1) svg.unpauseAnimations();
      else svg.pauseAnimations();
    }
    cancelAnimationFrame(frame);
    previous = 0;
    if (active() && speed !== 1 && svg.setCurrentTime) frame = requestAnimationFrame(tick);
  }
  function tick(now) {
    if (previous) svg.setCurrentTime(svg.getCurrentTime() + (now - previous) * speed / 1000);
    previous = now;
    frame = requestAnimationFrame(tick);
  }
  function update(value = {}) {
    if (typeof value.externalLinks === "boolean") externalLinks = value.externalLinks;
    if (typeof value.playing === "boolean") playing = value.playing;
    if (typeof value.visible === "boolean") visible = value.visible;
    if (Number.isFinite(value.speed) && value.speed > 0) speed = value.speed;
    if (typeof value.color === "string") document.body.style.color = value.color;
    if (value.scheme === "dark" || value.scheme === "light") document.documentElement.style.colorScheme = value.scheme;
    if (["contain", "cover", "fill"].includes(value.fit)) {
      svg.setAttribute("preserveAspectRatio", value.fit === "fill" ? "none" : `xMidYMid ${value.fit === "cover" ? "slice" : "meet"}`);
    }
    if (typeof value.interactive === "boolean") svg.style.pointerEvents = value.interactive ? "auto" : "none";
    if (Number.isFinite(value.restart)) {
      if (revision !== null && revision !== value.restart) {
        for (const animation of animations()) { animation.currentTime = 0; animation.play(); }
        if (svg.setCurrentTime) svg.setCurrentTime(0);
      }
      revision = value.restart;
    }
    sync();
    // Read-only application data; the document receives no engine/host APIs.
    window.dispatchEvent(new CustomEvent("bern:svg-state", { detail: { ...value, playing, visible, speed, restart: revision } }));
  }
  window.bernSvg = Object.freeze({ update });
  addEventListener("message", event => {
    if (event.source === parent && event.data?.type === "bern:svg") update(event.data);
  });
  document.addEventListener("visibilitychange", sync);
  document.addEventListener("animationstart", sync, true);
  new MutationObserver(sync).observe(svg, { attributes: true, attributeFilter: ["class"], childList: true, subtree: true });
  // Links must never navigate the containing application or SVG document.
  document.addEventListener("click", event => {
    const link=event.target.closest?.("a");
    if (!link) return;
    let url; try { url=new URL(link.getAttribute("href")); } catch {}
    if (!externalLinks || !url || url.protocol!=="https:" || !url.hostname) { event.preventDefault(); return; }
    if (parent !== window) { link.target="_blank"; link.rel="noopener noreferrer"; }
  }, true);
  sync();
})();
