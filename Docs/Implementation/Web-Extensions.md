**English** · [简体中文](../zh-CN/Implementation/Web-Extensions.md)

# The app's own browser extension

A browser target can name an adaptation layer that the app provides itself:

```toml
[targets.web_app]
platform = "browser"
ui = "web"
web_mode = "app"
web_extension = { path = "web", build = ["python3", "build.py"] }
```

This directory must contain `index.js`, whose default export is an initialization function. When `build` is written, Bern runs this command in that directory and appends the absolute path of the output directory as the last argument; the command must produce a deployable extension there, including `index.js`. Without `build`, Bern copies the source directory as is. `bern package` includes the entire generated extension. Once this setting is removed, the next build clears the stale extension output.

The generated loader imports the extension before starting the app. The initialization function receives: `pageFiles` (the table of browser `File` objects registered by the page), `text(ptr,len)` (copies a UTF-8 string out of WASM), `write(out,capacity,bytes)` (copies when the buffer is large enough, and returns the length needed), `defer(callback)` (runs after the current WASM call ends), and `media`:

- `player(id)` returns the executor's record.
- `command(command,id,value)` dispatches a command through the media hub. The commands are `open`, `play`, `pause`, `seek` (seconds), `set_volume`, `set_muted`.
- `stop(id)` detaches the executor and keeps this player's identity in the app.

The initialization function also receives `mode` (`app` or `ui`) and `remote.call(command, params)`. In WebUI, `remote.call` sends an app command over the existing, authenticated WebSocket and resolves with its JSON result; it rejects when the command fails, times out or the connection drops, and never resends any command automatically. The extension's request IDs are unrelated to those of the WASM client. `media.command` returns a Promise that resolves when the executor completes this operation.

The initialization function can return `imports` (grouped by WASM import namespace), `media: { name, create(id,report) }`, `start()` and `file(url, File)`, all optional. `start` runs after the engine connects and media is attached, before the window opens. When a file is picked, dropped or handed over by the system, Bern first waits for `file` to complete and then hands the URL to the app; the extension can use this to register the file's metadata with the native engine without uploading the file's contents. When file preparation is rejected, its error is shown and the unused URL is released. `prepareFile(url)` offers the same hook to places that bring in files programmatically.

The namespace `bern` is reserved for Bern. Import functions must copy borrowed WASM input before deferring execution; the initialization function itself runs before the WASM exports are available, and cannot read memory at that point.

The media record provides `video` (a persistent DOM element), `version` (the source version as a BigInt), and `load(url,version)`, `unload(version)`, `play()`, `pause()`, `seek(seconds)`, `setVolume(value)`, `setMuted(value)`, `release()`. These methods may return Promises, and Bern completes the request when they resolve. `load` must set `version` synchronously, run operations in order, and ignore work that belongs to a source that has already been replaced. Views move the same element and do not restart the executor. `report(version,state)` only reports observed state and never issues new commands: `phase` is a phase name from the media contract, `position` and the optional `duration` are in seconds, and `seekable`, `volume`, `muted` and the optional `error` complete the state.

Apps without an extension keep using the HTML media executor. A real app's `web/` adaptation layer is such an extension: business commands are still in Rust, while media and its GPU picture are handled by the playback core the app ships itself (Workers for WebCodecs, libass and danmaku). The generated WASM callbacks also report Rust panics to the browser console and the startup error page, instead of only showing `unreachable`.
