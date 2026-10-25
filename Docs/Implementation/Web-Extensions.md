# 应用自己的浏览器扩展

浏览器目标可以指定一个由应用自己提供的适配层：

```toml
[targets.web_app]
platform = "browser"
ui = "web"
web_mode = "app"
web_extension = { path = "web", build = ["python3", "build.py"] }
```

这个目录里必须有 `index.js`，默认导出一个初始化函数。写了 `build` 时，Bern 在该目录中执行这条命令，并把输出目录的绝对路径作为最后一个参数追加上去；命令要在那里生成可部署的扩展，包括 `index.js`。不写 `build` 时，Bern 原样复制源目录。`bern package` 会带上整个生成的扩展。去掉这项配置后，下次构建会清掉过期的扩展输出。

生成的加载器在启动应用之前导入扩展。初始化函数收到：`pageFiles`（页面登记的浏览器 `File` 表）、`text(ptr,len)`（复制一段 UTF-8 的 WASM 字符串）、`write(out,capacity,bytes)`（缓冲区放得下时复制，返回需要的长度）、`defer(callback)`（在当前这次 WASM 调用结束后执行），以及 `media`：

- `player(id)` 返回执行端的记录。
- `command(command,id,value)` 经媒体中枢派发命令。命令有 `open`、`play`、`pause`、`seek`（秒）、`set_volume`、`set_muted`。
- `stop(id)` 卸下执行端，保留应用里这个播放器的身份。

初始化函数还收到 `mode`（`app` 或 `ui`）与 `remote.call(command, params)`。在 WebUI 中，`remote.call` 经现有的、已认证的 WebSocket 发送一条应用命令，以它的 JSON 结果兑现；命令失败、超时或连接断开时拒绝，不会自动重发任何命令。扩展的请求编号与 WASM 客户端的编号互不相干。`media.command` 返回一个 Promise，在执行端完成这次操作时兑现。

初始化函数可以返回：`imports`（按 WASM 导入的命名空间分组）、`media: { name, create(id,report) }`、`start()` 与 `file(url, File)`，都是可选的。`start` 在引擎连上、媒体挂好之后、打开窗口之前执行。选择文件、拖放与系统交来文件时，先等 `file` 完成，再把地址交给应用；扩展可以借此向原生引擎登记文件的元数据，而不用上传文件内容。文件准备被拒绝时显示它的错误，并释放没用上的地址。`prepareFile(url)` 给以程序方式接入文件的地方提供同样的钩子。

命名空间 `bern` 保留给 Bern。导入函数要在推迟执行之前复制借来的 WASM 输入；初始化函数本身在 WASM 的导出可用之前运行，这时还不能读内存。

媒体记录提供：`video`（一个常驻的 DOM 元素）、`version`（BigInt 的来源版本），以及 `load(url,version)`、`unload(version)`、`play()`、`pause()`、`seek(seconds)`、`setVolume(value)`、`setMuted(value)`、`release()`。这些方法可以返回 Promise，Bern 在它们兑现时完成请求。`load` 必须同步设置 `version`、让操作依次执行，并忽略属于已被替换的来源的工作。视图移动同一个元素，不重启执行端。`report(version,state)` 只报告观察到的状态，从不发出新命令：`phase` 是媒体契约里的阶段名，`position` 与可选的 `duration` 以秒计，`seekable`、`volume`、`muted` 与可选的 `error` 补全状态。

没有扩展的应用继续用 HTML 的媒体执行端。一个实际应用的 `web/` 适配层就是这样的扩展：业务命令仍在 Rust 中，媒体和它的 GPU 画面由应用自带的播放内核（WebCodecs、libass 与弹幕的 Worker）负责。生成的 WASM 回调还会把 Rust 的 panic 报到浏览器控制台和启动错误页上，而不是只显示 `unreachable`。
