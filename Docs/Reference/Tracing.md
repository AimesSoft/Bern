# 性能追踪

更新：2026-10-21。框架自己在各个环节打点，用来定位界面卡在哪一步：命令在引擎里排队多久、执行多久，状态的快照、比较、编码各花多少，哪个字段有多大，界面线程多久开始处理、写入用了多久，结果在哪一帧上屏。设计见[架构文档的“调试与性能追踪”](../Design.md#调试与性能追踪)。

**实现进度**：开关、过滤、记录、汇总、时间线、`trace.*` 会话命令、`bern run --trace` 与引擎一侧（②③④、查询、模块调用、`span!`）、桥接一侧（解码、按字段编码、回调触发）已实现，在 Rust 测试中验证。各前端在界面线程一侧的打点（①⑤⑥、控件动作、帧节拍、长帧）还没有接入：现在只有引擎和桥接的段，交互链路与长帧要等前端接入后才会出现。

## 打开

规则的写法在三处相同：

| 方式 | 用法 |
| --- | --- |
| 环境变量 | `BERN_TRACE=engine,ui,slow:16ms`，应用启动时读取 |
| `bern run --trace <规则>` | 规则写错时在构建前报错。桌面程序与 WebUI 引擎宿主设为 `BERN_TRACE`；iOS、tvOS 模拟器经 `SIMCTL_CHILD_BERN_TRACE` 传入；Android 用 `am start --es bern.trace '<规则>'` 传给应用；`web_mode = "app"` 的页面在地址后加 `#bern-trace=<规则>`。后两种要由前端读出交给运行时，前端接入前不起作用，Android 现在用下面的系统属性 |
| 会话命令 | 运行中用 `trace.start`、`trace.stop`、`trace.report`，CLI 写作 `trace start --rules …`，MCP 工具同名 |

Android 不经 `bern run` 启动时，可以用 `adb shell setprop debug.bern.trace '<规则>'`（不超过 91 字节，release 构建也读取），对之后启动的应用生效，用 `adb shell setprop debug.bern.trace ''` 清掉。`web_mode = "app"` 的页面没有控制通道，运行中只能用浏览器控制台（前端接入后提供）。WebUI 的引擎宿主收到 `trace.*` 后自己记录引擎一侧，并转给页面记录界面一侧。

旧的调试变量是这套规则的别名：`BERN_FRAME_LOG`（非空）等于 `motion,frame,style`，`BERN_MOTION_LOG`（设置即可）等于 `motion.values`。WinUI 3 与 GTK 前端接入前，它们照旧由前端自己读取并打印原来的行，运行时同时按别名开启一次追踪。

## 规则

```text
规则   = 项 { "," 项 }
分类   = engine | ui | frame | span | motion | motion.values | style | view | all | -分类 | 1 | on
过滤   = command:<通配> | query:<通配> | control:<通配> | field:<通配> | feature:<契约.功能>
阈值   = slow:<时长> | long:<时长>
输出   = file:<路径> | file:none | quiet | buffer:<条数>
时长   = 数字 [us | ms | s]          不写单位按毫秒
```

| 分类 | 记录什么 |
| --- | --- |
| `engine` | 命令与查询的排队、执行、快照、比较、编码，模块之间的调用，以及 `span!` |
| `ui` | 界面线程：编码并派发命令、等待切到界面线程、解码并写入、控件动作与交互链路 |
| `frame` | 帧节拍与长帧 |
| `span` | 只记应用代码里的 `span!` |
| `motion` | 弹簧动画的汇总 |
| `motion.values` | 动画每次写出的变换，逐行打印（只在写出时打开，`all` 不含它） |
| `style` | WinUI 3 解析样式字典 |
| `view` | 预留：生成视图的重算 |

`1`、`on` 等于 `engine,ui,frame,span`；`all` 是除 `motion.values` 外的全部；`-frame` 这样的写法从中去掉一类。只写了过滤、没写分类时，按 `1` 记录。

过滤：同一种过滤之间取并集，不同种之间取交集。通配只认 `*`（任意一段字符，点也算）和 `?`（一个字符）。

| 过滤 | 作用 |
| --- | --- |
| `command:<通配>` | 只记录 id 匹配的命令与查询，例如 `command:library.*`、`command:*.add_item` |
| `query:<通配>` | 只匹配查询 |
| `control:<通配>` | 只列出在这些控件上开始的交互，控件身份是“组件名.控件 id”，如 `control:Library.add` |
| `field:<通配>` | 只逐个列出这些状态字段，其他字段合并成一行 |
| `feature:<名称>` | 展开成功能契约 `#[commands(…)]` 里的命令；写 `MediaFeature.Clip` 或只写 `Clip`。需要构建把功能表交给运行时，前端接入时一起接上 |

阈值：`slow:` 以下的交互和命令不打印汇总段，照常计入报告和时间线，默认 0（全部打印）；`long:` 是长帧阈值，默认 1.5 个刷新间隔（60 Hz 下 25 ms），前端不知道刷新间隔时用 25 ms。

输出：`file:` 指定时间线文件，默认写到临时目录（`BERN_IPC_DIR` 设了时写到那里）的 `bern-trace-<命名空间>-<进程号>.json`；`file:none` 不写。`quiet` 不打印汇总，报告与时间线照常。`buffer:` 是每个线程环形缓冲的条数，默认 32768 条（每条 64 字节），满了丢弃新记录并计数。

## 输出

### 终端汇总

桌面程序与模拟器打印到标准错误，Android 打印到 logcat（标签 `bern-trace`），网页打印到浏览器控制台。一次交互或一条命令一段，行文是英文，与 Bern 其他运行时输出一致。例（数字为毫秒）：

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

- 一个字段编码后不小于 64 KiB，或它的快照、比较、编码、写入加起来不少于 1 ms，标 `sent whole (large)`；列表字段给出项数。改动大列表中的一行时，整个列表要重新快照、比较和发送，这一行会把它标出来。
- `outside Bern`：同一段时间里界面线程上不是 Bern 分段的时间，即生成视图的重算加上系统 UI 框架的布局与绘制。
- 同一条命令一秒内超过 10 段时，其余的合并成每秒一行。
- 交互只在前端接入后出现；之前所有命令都按“命令段”打印。

### trace.report

返回 `TraceReport`（CLI 打印为 JSON，MCP 原样交给调用者）。时间单位为微秒：

| 字段 | 内容 |
| --- | --- |
| `running`、`window_ms`、`events`、`dropped` | 是否仍在记录、统计覆盖的时长、读到的记录数、丢弃数 |
| `commands` | 每条命令（按入口分开）的 `total_us`、`queue_us`、`exec_us`、`publish_us`、`apply_us`、`bytes`，各为 `{count, p50, p95, max}` |
| `queries` | 每个查询的 `queue_us`、`provider_us`（从开始到答复）、`round_trip_us`（从请求到界面显示）与 `cancelled` |
| `fields` | 变过的状态字段：次数、最大字节数、列表最多项数，快照、比较、编码、写入的分布 |
| `interactions` | 最慢的几次交互（`top`，默认 10），以及从其他入口来的最慢命令：控件、命令、总时长、修订号与各段 |
| `frames`、`long_frames` | 帧间隔的分布与长帧数；最近 50 个长帧，各自的 Bern 段、Bern 之外的时间与同期引擎在做什么 |
| `spans` | `span!` 与前端命名段按名称的分布 |

分位数取每项最近 4096 个样本，最大值是全程的。`reset: true` 在返回后清空统计。停止后仍可读取最后一次的报告。

### 时间线

Chrome trace event 的 JSON 数组格式，能直接在 ui.perfetto.dev 或 chrome://tracing 打开；格式允许省略结尾的 `]`，进程中途退出时文件仍能读入。每个线程一条轨道（`bern-engine`、界面线程等），帧单独一条轨道；同一请求从派发、执行到界面处理用箭头连起来；字段的字节数、列表项数、排队时间放在事件参数里。引擎跑在页面线程上的网页（`web_mode = "app"`）用一条虚拟轨道单独显示引擎。WebUI 的宿主和页面时钟不同源，各写一份，页面的那份由宿主在 `trace.stop` 时取回，存为 `…-page.json`。

`trace.stop` 传 `timeline: true` 时结果里带上整份时间线，给文件不在本机的情况用（Android、WebUI 页面）。

## 应用代码中的分段

```rust
let _span = bern::trace::span!("reindex");
let _span = bern::trace::span!("load {}", path); // 只在记录时格式化
```

分段在作用域结束时记录，出现在所在线程的轨道上；在命令中执行时也列在这条命令的汇总段里。追踪关闭时 `span!` 只做一次标志判断。

## 开销与线程

- 关闭时每个打点处只读一次原子标志（前端读自己的副本）。release 构建也包含这些打点。
- 打开时，打点处读单调时钟（Apple 为 `CLOCK_UPTIME_RAW`，Linux 与 Android 为 `CLOCK_MONOTONIC`，Windows 为性能计数器，网页为 `performance.now()`），把 64 字节的记录写进本线程的环形缓冲。汇总、打印与写文件都在后台线程 `bern-trace` 上进行；网页没有线程，由页面定时器读取。
- 跨线程靠命令 token、引擎任务序号与状态修订号对应；各线程的记录到达顺序不定，汇总器等到一批读完再对。

## 前端接口

前端经 C 接口记录界面线程一侧（GTK 直接调用同名的 Rust 函数）：

| 函数 | 作用 |
| --- | --- |
| `bern_trace_mask() -> u32` | 当前记录的分类位，0 表示关闭 |
| `bern_trace_now() -> u64` | 追踪时钟的纳秒数 |
| `bern_trace_ui(kind, key, a, b, t0, t1, name, name_len)` | 记录一段：`kind` 见下表 |
| `bern_trace_text(line, len)` | 原样打印一行（`motion.values`） |
| `bern_trace_configure(rules, len, dir, len)` | 在 `bern_app_start` 之前给出规则与时间线目录（Android、网页） |
| `bern_trace_pump()` | 读取并处理记录（网页的定时器调用） |

| `kind` | 段 | `key` | `a` | `b` |
| --- | --- | --- | --- | --- |
| 1 | 控件动作 | 交互号 | | |
| 2 | 编码并派发命令 | token | 命令下标 | 交互号 |
| 3 | 处理状态更新 | 修订号 | 字节数 | |
| 4 | 解码并写入一个字段 | 修订号 | 字段下标 | 字节数 |
| 5 | 处理命令答复 | token | | |
| 6 | 处理查询状态 | 订阅号 | 版本 | |
| 7 | 帧节拍：`t0` 节拍，`t1` 这一帧的工作结束 | 帧号 | 刷新间隔 ns | 上屏时刻，未知为 0 |
| 8 | 其他界面线程工作，`name` 命名 | | 分类位 | |

分类位：`engine` 1、`ui` 2、`frame` 4、`span` 8、`motion` 16、`motion.values` 32、`style` 64、`view` 128。帧节拍要由连续运行的节拍源给出（display link、Choreographer、帧时钟、`requestAnimationFrame`），只在真正绘制时才报的来源会把空闲误算成长帧。
