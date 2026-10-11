const { app, BrowserWindow } = require("electron");
const path = require("node:path");

app.whenReady().then(() => {
  const window = new BrowserWindow({ width: 480, height: 320, title: "Hello" });
  window.loadFile(path.join(__dirname, "index.html"));
});
app.on("window-all-closed", () => app.quit());
