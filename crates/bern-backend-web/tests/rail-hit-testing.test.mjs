// PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs node --test <this file>
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const playwright = process.env.PLAYWRIGHT_MODULE
  ? await import(process.env.PLAYWRIGHT_MODULE)
  : null;
const css = (await Promise.all(["bern", "features"].map(name =>
  readFile(new URL(`../templates/${name}.css`, import.meta.url), "utf8")))).join("\n");

for (const engine of ["chromium", "webkit"]) {
  test(`${engine}: rail decoration passes mouse/touch to the slider while chapter buttons remain clickable`, {
    skip: !playwright && "Set PLAYWRIGHT_MODULE to run the browser integration tests (required in CI)",
  }, async () => {
    const browser = await playwright[engine].launch({ headless: true,
      channel: engine === "chromium" ? process.env.PLAYWRIGHT_CHROMIUM_CHANNEL : undefined,
    });
    const page = await browser.newPage({ viewport: { width: 900, height: 300 }, hasTouch: true });
    try {
      for (const wrapped of [false, true]) {
        // The same positioned rail structures emitted for desktop and compact
        // timelines; native browser hit testing and range input handle the events.
        const marks = `<div class="bern-rail bern-rail-horizontal">
          <div class="bern-canvas bern-rail-item" style="--bern-rail-at:0.25;width:3px;height:4px"><canvas></canvas></div>
          <div class="bern-canvas bern-rail-item" style="--bern-rail-at:0.75;width:3px;height:4px"><canvas></canvas></div>
        </div>`;
        await page.setContent(`<style>${css}</style>
          <div style="position:relative;margin:20px;width:800px;height:120px">
            <input aria-label="Position" class="bern-slider" type="range" min="0" max="1" step="any" value="0.1" style="position:absolute;top:0;width:800px;height:44px">
            <div class="bern-positioned" style="top:20px;width:800px;height:4px">
              ${wrapped ? `<div class="bern-layers">${marks}</div>` : marks}
            </div>
            <div class="bern-positioned" style="top:60px;width:800px;height:24px">
              <div class="bern-rail bern-rail-horizontal">
                <button class="bern-rail-item" style="--bern-rail-at:0.5">Chapter</button>
              </div>
            </div>
          </div>`);
        await page.evaluate(() => {
          window.changes = [];
          document.querySelector("input").addEventListener("change", event => changes.push(Number(event.target.value)));
          document.querySelector("button").onclick = () => window.chapterClicked = true;
        });
        const slider = page.getByRole("slider", { name: "Position" });
        const r = await slider.boundingBox();
        // Empty center and the actual painted marks must all hit the input.
        assert.equal(await slider.evaluate(input => {
          const r = input.getBoundingClientRect();
          const points = [{ x: r.x + r.width / 2, y: r.y + r.height / 2 }];
          for (const mark of document.querySelectorAll(".bern-canvas")) {
            const m = mark.getBoundingClientRect();
            points.push({ x: m.x + m.width / 2, y: m.y + m.height / 2 });
          }
          return points.every(p => document.elementFromPoint(p.x, p.y) === input);
        }), true);
        await page.mouse.click(r.x + r.width / 2, r.y + r.height / 2);
        assert.ok(Math.abs(Number(await slider.inputValue()) - 0.5) < 0.02);
        await page.waitForFunction(() => Math.abs(changes.at(-1) - 0.5) < 0.02);
        await page.touchscreen.tap(r.x + r.width * 0.75, r.y + r.height / 2);
        assert.ok(Math.abs(Number(await slider.inputValue()) - 0.75) < 0.02);
        await page.mouse.move(r.x + r.width * 0.75, r.y + r.height / 2);
        await page.mouse.down();
        await page.mouse.move(r.x + r.width * 0.25, r.y + r.height / 2, { steps: 10 });
        await page.mouse.up();
        assert.ok(Math.abs(Number(await slider.inputValue()) - 0.25) < 0.02);
        await page.waitForFunction(() => Math.abs(changes.at(-1) - 0.25) < 0.02);
        await page.getByRole("button", { name: "Chapter" }).click();
        assert.equal(await page.evaluate(() => chapterClicked), true);
      }
    } finally {
      await browser.close();
    }
  });
}
