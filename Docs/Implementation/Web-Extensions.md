# Application browser extensions

A browser target can name an application-owned adapter:

```toml
[targets.web_app]
platform = "browser"
ui = "web"
web_mode = "app"
web_extension = { path = "web", build = ["python3", "build.py"] }
```

The directory must contain `index.js` exporting a default setup function. With `build`, Bern runs the command in that directory and appends the absolute output directory as the last argument. The command must produce the deployable extension there, including `index.js`. Without `build`, Bern mirrors the source directory. `bern package` includes the entire resulting extension. Removing the configuration removes stale extension output on the next build.

The generated loader imports the extension before starting the application. Setup receives `pageFiles` (the registered browser File map), `text(ptr,len)` (copy a UTF-8 WASM string), `write(out,capacity,bytes)` (copy if the buffer fits, return required length), `defer(callback)` (run after the current WASM call), and `media`:

- `player(id)` returns the executor's record.
- `command(command,id,value)` dispatches through the media hub. Commands are `open`, `play`, `pause`, `seek` (seconds), `set_volume`, `set_muted`.
- `stop(id)` unloads the executor while retaining the application player's identity.

Setup also receives `mode` (`app` or `ui`) and `remote.call(command, params)`. In WebUI, `remote.call` sends an application command through the existing authenticated WebSocket and resolves with its JSON result. It rejects on command failure, timeout or disconnect; no command is replayed automatically. Extension request IDs are isolated from the WASM client's IDs. `media.command` returns a promise that settles when the executor completes the operation.

Setup returns optional `imports`, keyed by WASM import namespace, optional `media: { name, create(id,report) }`, `start()` and `file(url, File)`. `start` runs after the engine connection and media attachment, before opening windows. File pick, drop and launch handlers await `file` before passing the URL to the application; extensions can register metadata with a native engine without uploading file bytes. A rejected file preparation displays its error and releases the unused URL. `prepareFile(url)` provides the same hook to programmatic file integrations.

The `bern` namespace is reserved. Imports must copy borrowed WASM input before deferring; setup itself runs before WASM exports are available and must not read memory yet.

A media record exposes `video` (a persistent DOM element), `version` (BigInt source version), and `load(url,version)`, `unload(version)`, `play()`, `pause()`, `seek(seconds)`, `setVolume(value)`, `setMuted(value)`, `release()`. Methods may return promises; Bern completes requests when they settle. `load` must set `version` synchronously, serialize operations and ignore work belonging to superseded sources. Views move the same element without restarting the executor. `report(version,state)` reports observations, never new commands: `phase` is a contract phase name, `position` and optional `duration` are seconds, and `seekable`, `volume`, `muted`, optional `error` complete the state.

Applications without an extension retain the HTML media executor. Erika's `web/` adapter is a consumer: its business commands remain in Rust while liberika's WebCodecs/libass/danmaku worker owns media and its GPU surface. The generated WASM callback also reports Rust panics to the browser console and startup error view, rather than showing only `unreachable`.
