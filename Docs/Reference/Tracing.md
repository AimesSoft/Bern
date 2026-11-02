**English** · [简体中文](../zh-CN/Reference/Tracing.md)

# Performance tracing

Updated: 2026-10-21. The framework instruments each stage itself, to find the step where the UI stalls: how long a command waits in the engine's queue and how long it runs, how much the state's snapshot, comparison and encoding each take, how large each field is, how long before the UI thread starts processing and how long the write takes, and on which frame the result reaches the screen. For the design, see ["Debugging and performance tracing" in the architecture document](../Design.md#debugging-and-performance-tracing).

**Implementation progress**: the switch, filters, recording, summaries, the timeline, the `trace.*` session commands, `bern run --trace`, the engine side (②③④, queries, calls between modules, `span!`) and the bridge side (decoding, per-field encoding, callback firing) are implemented and verified in Rust tests. The frontends' instrumentation on the UI thread side (①⑤⑥, control actions, frame ticks, long frames) is not yet wired in: for now there are only engine and bridge segments, and interaction chains and long frames appear only once the frontends are wired in.

## Turning it on

The rules are written the same way in all three places:

| Method | Usage |
| --- | --- |
| Environment variable | `BERN_TRACE=engine,ui,slow:16ms`, read when the app starts |
| `bern run --trace <rules>` | Malformed rules are reported as an error before the build. Desktop programs and the WebUI engine host get it as `BERN_TRACE`; iOS and tvOS simulators receive it through `SIMCTL_CHILD_BERN_TRACE`; on Android it is passed to the app with `am start --es bern.trace '<rules>'`; pages with `web_mode = "app"` get `#bern-trace=<rules>` appended to the address. The last two must be read by the frontend and handed to the runtime, so they have no effect until the frontends are wired in; Android currently uses the system property below |
| Session commands | `trace.start`, `trace.stop`, `trace.report` while running; in the CLI, `trace start --rules …`; the MCP tools have the same names |

When Android is not started through `bern run`, you can use `adb shell setprop debug.bern.trace '<rules>'` (at most 91 bytes, also read by release builds); it applies to apps started afterwards, and `adb shell setprop debug.bern.trace ''` clears it. Pages with `web_mode = "app"` have no control channel; while running, only the browser console can be used (provided once the frontend is wired in). The WebUI engine host, on receiving `trace.*`, records the engine side itself and forwards the command to the page to record the UI side.

The old debugging variables are aliases for these rules: `BERN_FRAME_LOG` (non-empty) equals `motion,frame,style`, and `BERN_MOTION_LOG` (merely being set) equals `motion.values`. Until the WinUI 3 and GTK frontends are wired in, these frontends still read them themselves and print the original lines, and the runtime also turns tracing on once according to the alias.

## Rules

```text
rules      = item { "," item }
category   = engine | ui | frame | span | motion | motion.values | style | view | all | -category | 1 | on
filter     = command:<pattern> | query:<pattern> | control:<pattern> | field:<pattern> | feature:<contract.feature>
threshold  = slow:<duration> | long:<duration>
output     = file:<path> | file:none | quiet | buffer:<count>
duration   = number [us | ms | s]          no unit means milliseconds
```

| Category | What it records |
| --- | --- |
| `engine` | Queueing, execution, snapshot, comparison and encoding of commands and queries, calls between modules, and `span!` |
| `ui` | The UI thread: encoding and dispatching commands, waiting to switch to the UI thread, decoding and writing, control actions and interaction chains |
| `frame` | Frame ticks and long frames |
| `span` | Only the `span!` in the app's code |
| `motion` | Summaries of spring animations |
| `motion.values` | Every transform an animation writes, printed line by line (only when written explicitly; `all` does not include it) |
| `style` | WinUI 3 resolving style dictionaries |
| `view` | Reserved: recomputation of generated views |

`1` and `on` equal `engine,ui,frame,span`; `all` is everything except `motion.values`; a form like `-frame` removes one category from it. When only filters are written and no category, recording follows `1`.

Filters: filters of the same kind are combined as a union, different kinds as an intersection. Patterns only recognize `*` (any run of characters, dots included) and `?` (one character).

| Filter | Effect |
| --- | --- |
| `command:<pattern>` | Records only commands and queries whose id matches, for example `command:library.*`, `command:*.add_item` |
| `query:<pattern>` | Matches only queries |
| `control:<pattern>` | Lists only interactions that start on these controls; a control's identity is "component name.control id", such as `control:Library.add` |
| `field:<pattern>` | Lists only these state fields one by one; the other fields are merged into one line |
| `feature:<name>` | Expands to the commands in the feature contract's `#[commands(…)]`; write `MediaFeature.Clip` or just `Clip`. Requires the build to hand the feature table to the runtime, which is wired up together with the frontends |

Thresholds: interactions and commands below `slow:` do not print a summary block, but are still counted in the report and the timeline; the default is 0 (print everything). `long:` is the long-frame threshold, by default 1.5 refresh intervals (25 ms at 60 Hz); when the frontend does not know the refresh interval, 25 ms is used.

Output: `file:` sets the timeline file, by default `bern-trace-<namespace>-<pid>.json` in the temporary directory (or in `BERN_IPC_DIR` when it is set); `file:none` writes none. `quiet` prints no summaries; the report and the timeline work as usual. `buffer:` is the number of entries in each thread's ring buffer, by default 32768 (64 bytes each); when it is full, new records are dropped and counted.

## Output

### Terminal summary

Desktop programs and simulators print to standard error, Android prints to logcat (tag `bern-trace`), and web pages print to the browser console. There is one block per interaction or command, and the text is in English, consistent with the rest of Bern's runtime output. Example (numbers in milliseconds):

```text
bern-trace: on engine,ui,frame,span, timeline /var/folders/…/bern-trace-showcase-4242.json
bern-trace: interaction Library.add 39.0 ms (action 0.20)
  showcase.add_item: dispatch 0.10, queue 0.20, exec 0.20, publish 4.60 -> rev 5
    publish: snapshot 1.60, compare 2.00, encode 0.90, other 0.10
    items 1.25 MiB, 10001 items, sent whole (large): snapshot 1.50, compare 1.90, encode 0.90, apply 6.60
    1 unchanged field: snapshot 0.10, compare 0.10
    to UI 0.20, apply 6.80
  on screen +26.8 (22.1 outside Bern)
bern-trace: command showcase.add_item (cli) 5.00 ms: queue 0.20, exec 0.20, publish 4.60 -> rev 6
bern-trace: long frame 60.0 ms at 12.84 s (refresh 16.7 ms): Bern 30.0 (apply rev 9 30.0), outside Bern 30.0; engine meanwhile: publish rev 10 5.00
bern-trace: showcase.set_volume x42 more within a second, total p50 0.40, p95 0.90, max 1.20 ms
```

- A field that is at least 64 KiB after encoding, or whose snapshot, comparison, encoding and write add up to at least 1 ms, is marked `sent whole (large)`; list fields give their item count. When one row of a large list changes, the whole list has to be snapshotted, compared and sent again, and this line points it out.
- `outside Bern`: time on the UI thread during the same period that is not in a Bern segment, that is, the recomputation of generated views plus layout and drawing by the system UI framework.
- When the same command has more than 10 blocks within one second, the rest are merged into one line per second.
- Interactions appear only once the frontends are wired in; until then, every command is printed as a "command block".

### trace.report

Returns a `TraceReport` (the CLI prints it as JSON, MCP hands it to the caller as is). Times are in microseconds:

| Field | Contents |
| --- | --- |
| `running`, `window_ms`, `events`, `dropped` | Whether recording is still running, the time span the statistics cover, the number of records read, the number dropped |
| `commands` | For each command (separated by entry point) `total_us`, `queue_us`, `exec_us`, `publish_us`, `apply_us`, `bytes`, each as `{count, p50, p95, max}` |
| `queries` | For each query `queue_us`, `provider_us` (from start to answer), `round_trip_us` (from request to display in the UI) and `cancelled` |
| `fields` | State fields that changed: count, maximum bytes, maximum list item count, and the distributions of snapshot, comparison, encoding and write |
| `interactions` | The slowest interactions (`top`, default 10), and the slowest commands from other entry points: control, command, total time, revision and each segment |
| `frames`, `long_frames` | The distribution of frame intervals and the number of long frames; the last 50 long frames, each with its Bern segments, the time outside Bern and what the engine was doing at the same time |
| `spans` | The distribution of `span!` and frontend named segments, by name |

Percentiles use the last 4096 samples of each item; the maximum covers the whole run. `reset: true` clears the statistics after returning. After stopping, the last report can still be read.

### Timeline

The format is the JSON array format of Chrome trace events, which opens directly in ui.perfetto.dev or chrome://tracing; the format allows omitting the closing `]`, so the file can still be loaded when the process exits midway. Each thread gets one track (`bern-engine`, the UI thread and so on), and frames get a track of their own; the same request is connected by arrows from dispatch through execution to UI processing; a field's byte count, list item count and queueing time are placed in the event arguments. Web pages whose engine runs on the page thread (`web_mode = "app"`) show the engine separately on a virtual track. The WebUI host and page do not share a clock source, so each writes its own file; the page's file is fetched by the host at `trace.stop` and saved as `…-page.json`.

When `trace.stop` is passed `timeline: true`, the result includes the entire timeline, for cases where the file is not on the local machine (Android, WebUI pages).

## Spans in app code

```rust
let _span = bern::trace::span!("reindex");
let _span = bern::trace::span!("load {}", path); // formatted only while recording
```

A span is recorded when its scope ends and appears on its thread's track; when it runs inside a command, it is also listed in that command's summary block. When tracing is off, `span!` performs only one flag check.

## Overhead and threads

- When off, each instrumentation point reads one atomic flag (the frontends read their own copy). Release builds also contain these instrumentation points.
- When on, an instrumentation point reads the monotonic clock (`CLOCK_UPTIME_RAW` on Apple, `CLOCK_MONOTONIC` on Linux and Android, the performance counter on Windows, `performance.now()` on the web) and writes a 64-byte record into the current thread's ring buffer. Summarizing, printing and file writing all happen on the background thread `bern-trace`; web pages have no threads, so a page timer reads the records.
- Records across threads are matched by command token, engine task sequence number and state revision; records from different threads arrive in no fixed order, so the aggregator waits until a batch has been read before matching.

## Frontend interface

Frontends record the UI thread side through a C interface (GTK calls the Rust functions of the same names directly):

| Function | Purpose |
| --- | --- |
| `bern_trace_mask() -> u32` | The category bits currently recorded; 0 means off |
| `bern_trace_now() -> u64` | The tracing clock in nanoseconds |
| `bern_trace_ui(kind, key, a, b, t0, t1, name, name_len)` | Records one segment: see the table below for `kind` |
| `bern_trace_text(line, len)` | Prints one line as is (`motion.values`) |
| `bern_trace_configure(rules, len, dir, len)` | Gives the rules and the timeline directory before `bern_app_start` (Android, web pages) |
| `bern_trace_pump()` | Reads and processes records (called by the web page's timer) |

| `kind` | Segment | `key` | `a` | `b` |
| --- | --- | --- | --- | --- |
| 1 | Control action | Interaction number | | |
| 2 | Encode and dispatch a command | token | Command index | Interaction number |
| 3 | Process a state update | Revision | Byte count | |
| 4 | Decode and write one field | Revision | Field index | Byte count |
| 5 | Process a command reply | token | | |
| 6 | Process query state | Subscription number | Version | |
| 7 | Frame tick: `t0` the tick, `t1` the end of this frame's work | Frame number | Refresh interval in ns | Presentation time, 0 if unknown |
| 8 | Other UI thread work, named by `name` | | Category bits | |

Category bits: `engine` 1, `ui` 2, `frame` 4, `span` 8, `motion` 16, `motion.values` 32, `style` 64, `view` 128. Frame ticks must come from a continuously running tick source (display link, Choreographer, the frame clock, `requestAnimationFrame`); a source that reports only when it actually draws makes idle time count as long frames.
