**English** · [简体中文](../zh-CN/Guides/App-CLI-and-Skills.md)

# App CLI & AI Skills (`SKILL.md`)

Because every UI action and business operation in Bern goes through a single typed command catalog, enabling the control channel gives you both a companion **app CLI** and a generated **`SKILL.md`** for AI agents and automated testing.

## 1. Enabling the control channel

Add `channels = ["ipc"]` to any target in `Bern.toml`:

```toml
[targets.macos]
platform = "macos"
channels = ["ipc"]
```

When built (`bern build` / `bern run` / `bern package`), Bern outputs:

- **App CLI**: `target/bern/<target>/bin/<app>` (and inside the shipped bundle on desktop, such as `Contents/Helpers/<app>` on macOS).
- **Agent skill**: `target/bern/<target>/skills/<app>/SKILL.md` documenting every built-in UI/window/media command and every business command/query from your protocol.

## 2. Inspecting and driving mounted controls

Always use the CLI from the same build as the running app. Agents and scripts inspect mounted controls by semantic target name rather than brittle screen coordinates:

```sh
# List every currently mounted control, its role, label, value, and enabled state
my_app ui inspect

# Operate controls through the exact same code path as a user click or input
my_app ui activate --target tasks_view.add
my_app ui set-bool --target tasks_view.only_open --value true
my_app ui edit --target tasks_view.new_task --value "Ship v1.0"
my_app ui submit --target tasks_view.new_task
my_app ui select --target main_window.pages --option Settings
my_app ui set-language --language en
```

If a control is disabled, covered by a modal dialog, or not currently mounted, the CLI returns a structured error code (`target_unavailable`, `blocked_by_modal`, `target_ambiguous`, etc.) instead of silently failing.

## 3. Window management and screenshots

Built-in window commands query actual OS window state, test close confirmation flows, and capture window screenshots directly without full-screen recording:

```sh
my_app window list
my_app window open --kind settings
my_app window screenshot --output shot.png

# Trigger and respond to close confirmation protocols
my_app window close --window settings#1
my_app window confirm-close --window settings#1 --confirmation settings#1:c1 --decision Save
```

## 4. Calling engine commands, queries, and state directly

Every `#[commands]` and `#[queries]` method declared in your protocol becomes a typed subcommand under your protocol namespace:

```sh
my_app catalog                              # Print the full command & query catalog
my_app state                                # Read the live engine state snapshot
my_app tasks add --title "Review PR"        # Execute a protocol command
my_app tasks search --text "Review"         # Run a protocol query
```

## 5. Multi-engine targeting, mobile & SSH

- **Multiple running instances**: if the same bundle id is running on macOS and in the iOS simulator simultaneously, pass `--engine ios` or `--engine macos` (or `--engine watchos` / `--engine tvos` / `--engine wearos`).
- **Custom IPC directory**: set `BERN_IPC_DIR=/path/to/dir` on both the app and the CLI to isolate test runs. On Apple simulators, `bern run` forwards this automatically via `SIMCTL_CHILD_BERN_IPC_DIR`.
- **Android / Wear OS**: run the CLI on your development machine (never on the phone/watch); `bern run` automatically sets up `adb forward`.
- **Web (`web_app` / `web_ui`)**: for `web_app` with `channels = ["ipc"]`, `bern run` starts a local bridge to the open browser tab; static production web archives exclude the local CLI and `SKILL.md`.
- **Remote control over SSH**: run the CLI on the remote host under the account owning the IPC socket — no extra network port is needed:

```sh
ssh HOST /path/to/my_app state
```

## 6. In-process scripting (`bern.cli`)

If you want user scripts or plugins inside the application process to call the same command catalog with host-enforced allowlists and denylists, see [JavaScript & `bern.cli`](../Topics/JavaScript.md).
