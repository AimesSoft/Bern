import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function runtime() {
  const listeners = {}, frames = new Map();
  let next = 0, observer;
  const makeAnimation = (state = "running") => ({
    playState: state, currentTime: 300, playbackRate: 1, pending: false,
    pause() { this.playState = "paused"; }, play() { this.playState = "running"; }
  });
  const animations = [makeAnimation(), makeAnimation("paused")];
  const svg = { time: 2, paused: false, style: {}, attrs: {},
    getCurrentTime() { return this.time; }, setCurrentTime(t) { this.time = t; },
    pauseAnimations() { this.paused = true; }, unpauseAnimations() { this.paused = false; },
    setAttribute(k, v) { this.attrs[k] = v; }
  };
  const document = { hidden: false, body: { style: {} }, documentElement: { style: {} },
    querySelector() { return svg; }, getAnimations() { return animations; },
    addEventListener(k, fn) { listeners[k] = fn; }
  };
  const window = {}, parent = {};
  vm.runInNewContext(readFileSync(new URL("../src/runtime.js", import.meta.url), "utf8"), {
    window, parent, document,
    addEventListener(k, fn) { listeners[k] = fn; },
    requestAnimationFrame(fn) { frames.set(++next, fn); return next; },
    cancelAnimationFrame(id) { frames.delete(id); },
    MutationObserver: class { constructor(fn) { observer = fn; } observe() {} }
  });
  return { svg, animations, document, parent, listeners, frames,
    update: window.bernSvg.update, makeAnimation,
    mutation() { observer(); },
    tick(now) { const work = [...frames.values()]; frames.clear(); work.forEach(fn => fn(now)); }
  };
}

test("pause and resume CSS/SMIL without resuming author-paused animations", () => {
  const r = runtime();
  r.update({ playing: false });
  assert.equal(r.animations[0].playState, "paused");
  assert.equal(r.svg.paused, true);
  r.update({ playing: true });
  assert.equal(r.animations[0].playState, "running");
  assert.equal(r.animations[1].playState, "paused");
  assert.equal(r.svg.paused, false);
});
test("new script-created animations inherit pause and speed", () => {
  const r = runtime();
  r.update({ playing: false, speed: 2 });
  const next = r.makeAnimation(); r.animations.push(next); r.mutation();
  assert.equal(next.playState, "paused");
  assert.equal(next.playbackRate, 2);
});
test("SMIL speed advances document time and stops offscreen", () => {
  const r = runtime();
  r.update({ speed: 2 }); r.tick(1000); r.tick(1500);
  assert.equal(r.svg.time, 3);
  r.update({ visible: false });
  assert.equal(r.frames.size, 0);
  r.update({ visible: true }); r.tick(5000); r.tick(5500);
  assert.equal(r.svg.time, 4, "hidden time must not cause a jump");
  r.update({ speed: 1 }); assert.equal(r.frames.size, 0);
});
test("restart revisions replay CSS/SMIL, retaining the paused state", () => {
  const r = runtime();
  r.update({ restart: 0 });
  assert.equal(r.animations[0].currentTime, 300);
  r.update({ playing: false, restart: 1 });
  assert.equal(r.svg.time, 0);
  assert.equal(r.animations[0].currentTime, 0);
  assert.equal(r.animations[0].playState, "paused");
});
test("only parent messages control an isolated document", () => {
  const r = runtime();
  r.listeners.message({ source: {}, data: { type: "bern:svg", playing: false } });
  assert.equal(r.svg.paused, false);
  r.listeners.message({ source: r.parent, data: { type: "bern:svg", playing: false } });
  assert.equal(r.svg.paused, true);
});
test("invalid rates are ignored; cover/fill and page visibility work", () => {
  const r = runtime();
  r.update({ speed: -1, fit: "cover", interactive: false });
  assert.equal(r.animations[0].playbackRate, 1);
  assert.equal(r.svg.attrs.preserveAspectRatio, "xMidYMid slice");
  assert.equal(r.svg.style.pointerEvents, "none");
  r.document.hidden = true; r.listeners.visibilitychange();
  assert.equal(r.animations[0].playState, "paused");
  r.document.hidden = false; r.listeners.visibilitychange();
  assert.equal(r.animations[0].playState, "running");
});
