# Bern GPUI 触摸输入补丁

此处保留 crates.io 发布源码中的 `gpui-pre`、`gpui-pre-linux`、
`gpui-pre-windows` **0.3.10**。包元数据标明上游快照为 Zed `08cbb88`。
各包的原作者声明与 `LICENSE-APACHE` 均保留。上游为
https://github.com/zed-industries/zed 中的 `crates/gpui`、
`crates/gpui_linux`、`crates/gpui_windows`。

修改仅涉及输入派发：

- 核心：识别前派发原始 `TouchEvent`，监听器可等越过阈值才认领；认领时取消原识别器，不生成误点击或惯性。
- Wayland：绑定 `wl_touch`，保留触点起始 surface，派发精确逻辑坐标，seat 或设备失效时取消。
- X11：接收 XI 2.2 直接触屏事件，跟踪触点并去除其兼容鼠标重复事件，不拦截触控板。
- Windows：派发 `WM_POINTER` 触屏联系点，维持触点身份，丢失捕获或窗口时取消，并去除兼容鼠标重复事件。

Bern 运行时及生成应用的根 manifest 都写入对应 patch；Cargo 不会自动采用依赖 manifest 内的 patch。
不改 Cargo registry 缓存；本目录不保留构建输出、Cargo.lock、registry 缓存元数据或 Git 元数据。
